---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) سے موافقت پذیر۔

# فراہم کنندہ داخلہ اور شناخت کی پالیسی SoraFS (مسودہ SF-2B)

یہ یادداشت ** SF-2B ** کی عملی فراہمی کی دستاویز کرتی ہے: داخلہ راستہ کی تعریف ، شناختی تقاضے ، اور پے لوڈ۔
SoraFS اور اس کی درخواست میں اسٹوریج فراہم کرنے والوں کے لئے توثیق۔ یہ آر ایف سی میں بیان کردہ اعلی سطحی عمل میں توسیع کرتا ہے
فن تعمیر SoraFS اور باقی کام کو ٹریک ایبل انجینئرنگ کے کاموں میں توڑ دیتا ہے۔

## پالیسی کے مقاصد

- اس بات کو یقینی بنائیں کہ صرف جائزہ لینے والے آپریٹرز `ProviderAdvertV1` ریکارڈ شائع کرسکتے ہیں جو نیٹ ورک کے ذریعہ قبول کیے جاتے ہیں۔
- ہر اشتہار کی کلید کو گورننس سے منظور شدہ شناختی دستاویز ، توثیق شدہ اختتامی نکات ، اور کم سے کم حصص کی شراکت سے لنک کریں۔
۔
- عزم یا ایرگونومکس ٹولز کو توڑے بغیر ہنگامی تجدید اور منسوخی کی حمایت کریں۔

## شناخت اور داؤ کی ضروریات

| ضرورت | تفصیل | آؤٹ پٹ |
| --------- | ------- | ---------- |
| اشتہار کلیدی ذریعہ | فراہم کنندگان کو ایک کلیدی جوڑی ED25519 کو رجسٹر کرنا ہوگا جس میں ہر اشتہار کی علامت ہوتی ہے۔ قبولیت کا پیکٹ عوامی کلید کو گورننس کے دستخط کے ساتھ محفوظ کرتا ہے۔ | `ProviderAdmissionProposalV1` اسکیما کو `advert_key` (32 بائٹس) کے ساتھ بڑھانا اور اسے رجسٹر (`sorafs_manifest::provider_admission`) سے حوالہ دینا۔ |
| داؤ اشارے | قبولیت کی ضرورت ہے ایک غیر صفر `StakePointer` ایک فعال اسٹیکنگ پول کی نشاندہی کرتا ہے۔ | `sorafs_manifest::provider_advert::StakePointer::validate()` میں چیکنگ شامل کریں اور CLI/ٹیسٹوں میں غلطیاں دکھائیں۔ |
| دائرہ اختیار ٹیگز | فراہم کرنے والے دائرہ اختیار + قانونی رابطے کا اعلان کرتے ہیں۔ | `jurisdiction_code` (ISO 3166-1 الفا -2) اور اختیاری `contact_uri` کے ساتھ مجوزہ اسکیم کو وسعت دیں۔ |
| اختتامی نقطہ کی توثیق | ہر مشتہر اختتامی نقطہ کو ایم ٹی ایل ایس یا کوئیک سرٹیفکیٹ رپورٹ کے ذریعہ تعاون کرنا چاہئے۔ | Norito `EndpointAttestationV1` پے لوڈ کی وضاحت اور قبول پیکٹ کے اندر ہر اختتامی نقطہ کے لئے اسٹور کی گئی ہے۔ |

## داخلہ ورک فلو

1. ** تجویز بنائیں **
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...` شامل کریں
     `ProviderAdmissionProposalV1` + توثیق پیکیج تیار کرنے کے لئے۔
   - توثیق: مطلوبہ فیلڈز ، اسٹیک> 0 ، اور چنکر ہینڈل کو یقینی بنائیں `profile_id` میں معیاری ہیں۔
2. ** گورننس کی منظوری **
   - موجودہ لفافے کے ٹولز کا استعمال کرتے ہوئے بورڈ `blake3("sorafs-provider-admission-v1" || canonical_bytes)` پر دستخط کرتا ہے
     (ماڈیول `sorafs_manifest::governance`)۔
   - لفافہ `governance/providers/<provider_id>/admission.json` میں محفوظ کیا گیا ہے۔
3. ** لاگ انٹری **
   - مشترکہ جائز (`sorafs_manifest::provider_admission::validate_envelope`) کو نافذ کریں جو Torii/گیٹس/CLI کو دوبارہ استعمال کرتا ہے۔
   - Torii میں قبولیت کے راستے کو اپ ڈیٹ کیا گیا تاکہ ان اشتہارات کو مسترد کیا جاسکے جن کی ڈائجسٹ یا اختتامی تاریخ لفافے سے مختلف ہوتی ہے۔
4. ** تجدید اور منسوخی **
   - اختیاری اختتامی نقطہ/اسٹیک اپڈیٹس کے ساتھ `ProviderAdmissionRenewalV1` شامل کیا گیا۔
   - دستیابی سی ایل آئی پاتھ `--revoke` منسوخی کی وجہ ریکارڈ کرتا ہے اور گورننس ایونٹ کو آگے بڑھاتا ہے۔

## نفاذ کے کام

| ڈومین | ٹاسک | مالک (زبانیں) | حیثیت |
| -------- | ------- | ---------- | -------- |
| اسکیما | `ProviderAdmissionProposalV1` ، `ProviderAdmissionEnvelopeV1` ، اور `EndpointAttestationV1` (Norito) کی تعریف `crates/sorafs_manifest/src/provider_admission.rs` کے اندر۔ `sorafs_manifest::provider_admission` کے اندر توثیق ایڈز کے ساتھ نافذ کیا گیا ہے اسٹوریج / گورننس | ✅ مکمل |
| سی ایل آئی ٹولز | `sorafs_manifest_stub` کو سب کامنڈس کے ساتھ بڑھاؤ: `provider-admission proposal` ، `provider-admission sign` ، `provider-admission verify`۔ | ٹولنگ ڈبلیو جی | ✅ |سی ایل آئی پائپ لائن اب انٹرمیڈیٹ سرٹیفکیٹ پیکیجز (`--endpoint-attestation-intermediate`) کی حمایت کرتی ہے ، تجویز/لفافے کے لئے معیاری بائٹس جاری کرتی ہے ، اور `sign`/`verify` کے دوران بورڈ کے دستخطوں کی تصدیق کرتی ہے۔ آپریٹرز اشتہاری اداروں کو براہ راست فراہم کرسکتے ہیں یا دستخط شدہ اشتہارات کو دوبارہ استعمال کرسکتے ہیں ، اور آسان آٹومیشن کے لئے دستخطی فائلیں `--council-signature-public-key` اور `--council-signature-file` کے امتزاج کے ذریعے پاس کی جاسکتی ہیں۔

### سی ایل آئی حوالہ

`cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...` کے ذریعے ہر کمانڈ پر عمل کریں۔- `proposal`
  - مطلوبہ جھنڈے: `--provider-id=<hex32>` ، `--chunker-profile=<namespace.name@semver>` ،
    `--stake-pool-id=<hex32>` ، `--stake-amount=<amount>` ، `--advert-key=<hex32>` ،
    `--jurisdiction-code=<ISO3166-1>` ، اور کم از کم ایک `--endpoint=<kind:host>`۔
  - ہر اختتامی نقطہ کی توثیق کے لئے `--endpoint-attestation-attested-at=<secs>` کی ضرورت ہوتی ہے ،
    `--endpoint-attestation-expires-at=<secs>` ، کراس سرٹیفکیٹ
    `--endpoint-attestation-leaf=<path>` (سیریز میں ہر آئٹم کے لئے اختیاری `--endpoint-attestation-intermediate=<path>` کے ساتھ) اور کوئی بھی مذاکرات شدہ ALPN شناخت کنندگان
    (`--endpoint-attestation-alpn=<token>`)۔ کوئیک اختتامی نکات کراس ٹرانزٹ رپورٹس فراہم کرسکتے ہیں
    `--endpoint-attestation-report[-hex]=...`۔
  - آؤٹ پٹ: تجویز کے لئے معیاری بائٹس Norito (`--proposal-out`) اور JSON خلاصہ
    (پہلے سے طے شدہ stdout یا `--json-out`)۔
-`sign`
  - ان پٹ: تجویز کردہ (`--proposal`) ، اشتہار کی جگہ (`--advert`) ، اختیاری اشتہاری باڈی
    (`--advert-body`) ، دور برقرار رکھنے ، اور کم از کم ایک بورڈ کے دستخط۔ دستخطوں کو منظور کیا جاسکتا ہے
    ان لائن (`--council-signature=<signer_hex:signature_hex>`) یا فائلوں کے ذریعے استعمال کریں
    `--council-signature-public-key` کے ساتھ `--council-signature-file=<path>`۔
  - ایک لفافہ تصدیق کنندہ (`--envelope-out`) اور ایک JSON رپورٹ جو ڈائجسٹ لنکس ، دستخط کنندگان کی تعداد اور ان پٹ راہوں کو دکھاتا ہے۔
- `verify`
  - مماثل تجویز ، اشتہار ، یا اشتہاری باڈی کے لئے اختیاری چیک کے ساتھ موجودہ لفافے (`--envelope`) کی جانچ پڑتال کرتا ہے۔ JSON رپورٹ ڈائجسٹ اقدار ، دستخطی توثیق کی حیثیت ، اور کسی بھی مماثل اختیاری نوادرات کو اجاگر کرتی ہے۔
-`renewal`
  - ایک لفافہ ایک نئے سرٹیفکیٹ کو پہلے سے منظور شدہ ڈائجسٹ سے جوڑتا ہے۔ ضرورت ہے
    `--previous-envelope=<path>` اور اگلا `--envelope=<path>` (دونوں پے لوڈ Norito)۔
    سی ایل آئی اس بات کی تصدیق کرتی ہے کہ اسٹیک ، اختتامی نقطہ ، اور میٹا ڈیٹا کی تازہ کاریوں کی اجازت دیتے ہوئے پروفائل عرفی ، صلاحیتوں اور اشتہاری چابیاں میں کوئی تبدیلی نہیں ہے۔ معیاری بائٹس تیار کرتا ہے
    `ProviderAdmissionRenewalV1` (`--renewal-out`) JSON کا خلاصہ شامل کریں۔
- `revoke`
  - کسی ایسے فراہم کنندہ کے لئے ایمرجنسی پیکیج `ProviderAdmissionRevocationV1` جاری کرتا ہے جس کا لفافہ کھینچنا ضروری ہے۔ `--envelope=<path>` ، `--reason=<text>` ، کم از کم ایک بورڈ دستخط کی ضرورت ہے
    `--council-signature` ، `--revoked-at`/`--notes` اختیاری ہیں۔ سی ایل آئی کی نشاندہی اور ڈائجسٹ کی منسوخی کی جانچ پڑتال کرتی ہے ، Norito پے لوڈ کو `--revocation-out` کے ذریعے لکھتی ہے اور ہضم اور دستخطوں کی تعداد کے ساتھ JSON رپورٹ پرنٹ کرتی ہے۔
| توثیق | Torii اور گیٹ ویز اور `‎sorafs-node` کے ذریعہ استعمال ہونے والے مشترکہ توثیق کنندہ کا نفاذ۔ یونٹ ٹیسٹ + سی ایل آئی انضمام فراہم کریں نیٹ ورکنگ TL/اسٹوریج | ✅ مکمل |
| انضمام Torii | Torii پر ایڈورٹ انٹری میں تصدیق کنندہ کو پاس کریں اور پالیسی سے باہر اشتہارات کو مسترد کریں اور ٹیلی میٹری جاری کریں۔ | نیٹ ورکنگ TL | ✅ مکمل | Torii اب گورننس لفافے (`torii.sorafs.admission_envelopes_dir`) کو لوڈ کرتا ہے اور ان پٹ کے دوران ہضم/دستخطی میچ چیک کرتا ہے اور ٹیلی میٹری کو نمایاں کرتا ہے قبولیت۔ 【f: کریٹس/اروہ_ٹوری/ایس آر سی/سرفس/داخلہ
| تجدید | تجدید/منسوخی اسکیم + سی ایل آئی مددگار شامل کریں اور دستاویزات میں لائف سائکل گائیڈ شائع کریں (`provider-admission renewal`/`revoke` میں نیچے رن بک اور سی ایل آئی کمانڈز دیکھیں)۔ اسٹوریج / گورننس | ✅ مکمل || ٹیلی میٹری | پینلز/الرٹس کی تعریف `provider_admission` (تجدید غائب ، لفافے کی میعاد ختم ہوگئی)۔ | مشاہدہ | 🟠 چل رہا ہے | کاؤنٹر `torii_sorafs_admission_total{result,reason}` موجود ؛ زیر التواء پینل/الرٹس

### تجدید اور منسوخی کے لئے گائیڈ

#### شیڈول تجدید (داؤ/ٹوپولوجی کی تازہ کاری)
1. `provider-admission proposal` اور `provider-admission sign` کا استعمال کرتے ہوئے اس کے بعد کی تجویز/اشتہاری جوڑی بنائیں ، `--retention-epoch` میں اضافہ کریں اور ضرورت کے مطابق اسٹیک/اختتامی نقطہ کو اپ ڈیٹ کریں۔
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
   کمانڈ `AdmissionRecord::apply_renewal` کے ذریعے پاور/فائل فیلڈز کی مستقل مزاجی کی جانچ پڑتال کرتا ہے ، `ProviderAdmissionRenewalV1` کو جاری کرتا ہے ، اور لاگ کے لئے ڈائجسٹ پرنٹ کرتا ہے۔ گورننس۔ 【کریٹس/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#l477 】【 f: creats/sorafs_manifest/src/provider_admission.rs#l422】
3. `torii.sorafs.admission_envelopes_dir` میں پچھلے لفافے کو تبدیل کریں ، گورننس ریپوزٹری میں تجدید Norito/JSON انسٹال کریں ، اور `docs/source/sorafs/migration_ledger.md` میں تجدید ہیش + برقرار رکھنے کے دور کو شامل کریں۔
4. آپریٹرز کو مطلع کریں کہ نیا لفافہ متحرک ہوچکا ہے اور اندراج کی تصدیق کے لئے `torii_sorafs_admission_total{result="accepted",reason="stored"}` کی نگرانی کرتا ہے۔
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` کے ذریعے معیاری فکسچر کو دوبارہ تخلیق کریں اور انسٹال کریں۔ CI (`ci/check_sorafs_fixtures.sh`) Norito آؤٹ پٹ کے استحکام کی جانچ کرتا ہے۔

#### ایمرجنسی منسوخی
1. سمجھوتہ کرنے والے لفافے کی شناخت کریں اور منسوخ جاری کریں:
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
   CLI اشارے `ProviderAdmissionRevocationV1` ، `verify_revocation_signatures` کے ذریعے طے شدہ دستخط کی تصدیق کرتا ہے ، اور رپورٹ ڈائجسٹ منسوخی
2. قبولیت کیچوں میں منسوخی کے لئے `torii.sorafs.admission_envelopes_dir` ، Norito/JSON سے لفافے کو ہٹا دیں ، اور گورننس رپورٹس میں ہیش کو لاگ ان کریں۔
3. مانیٹر `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` اس بات کی تصدیق کرنے کے لئے کہ کیچز منسوخ شدہ اشتہار کو چھوڑ دیتے ہیں۔ واقعے کے جائزوں میں منسوخی کے نشانات رکھیں۔

## ٹیسٹ اور ٹیلی میٹری

- ذیل میں تجاویز اور داخلے کے احاطے میں سنہری فکسچر شامل کریں
  `fixtures/sorafs_manifest/provider_admission/`۔
- تجاویز کو دوبارہ تخلیق کرنے اور لفافوں کو چیک کرنے کے لئے CI (`ci/check_sorafs_fixtures.sh`) کو بڑھانا۔
- معیاری ہضم کے ساتھ `metadata.json` سے تیار کردہ فکسچر شامل ہیں۔ بہاو ​​ٹیسٹ اس کی تصدیق کرتے ہیں
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`۔
- انضمام کے ٹیسٹ فراہم کرنا:
  - Torii گمشدہ یا ختم ہونے والے لفافوں کے ساتھ اشتہارات کو قبول کرنے سے انکار کرتا ہے۔
  - CLI تجویز → لفافہ → توثیق کا ایک گول سفر انجام دیتا ہے۔
  - گورننس کی تجدید فراہم کرنے والے ID کو تبدیل کیے بغیر اختتامی نقطہ کی توثیق پر رول کرتی ہے۔
- ٹیلی میٹک تقاضے:
  - Torii میں `provider_admission_envelope_{accepted,rejected}` کاؤنٹرز کا ورژن۔ ✅ `torii_sorafs_admission_total{result,reason}` قبول/مسترد نتائج دکھاتا ہے۔
  - ڈیش بورڈز میں میعاد ختم ہونے والے انتباہات شامل کریں (7 دن کے اندر تجدید کی تجدید)۔

## اگلے اقدامات

1. ✅ Norito اسکیما میں تبدیلیوں کو حتمی شکل دی گئی ہے اور `sorafs_manifest::provider_admission` میں تصدیق شدہ ایڈز شامل کی گئیں۔ کسی بھی خصوصیات کی ضرورت نہیں ہے۔
2. ✅ CLI راستے (`proposal` ، `sign` ، `verify` ، `renewal` ، `revoke`) انضمام ٹیسٹ کے ذریعے دستاویزی اور جانچ کی گئی ہیں۔ رن بک کے ساتھ ہم آہنگی میں گورننس اسکرپٹ رکھیں۔
3. ✅ Torii داخلہ/دریافت لفافے میں داخل ہوتا ہے اور قبولیت/مسترد ہونے کے لئے ٹیلی میٹک کاؤنٹرز دکھاتا ہے۔
4. مشاہدے پر فوکس کریں: قبولیت پینل/انتباہات کو ختم کریں تاکہ وہ سات دن کے اندر اندر ہونے پر انتباہات کو متحرک کریں (`torii_sorafs_admission_total` ، میعاد ختم گیجز)۔