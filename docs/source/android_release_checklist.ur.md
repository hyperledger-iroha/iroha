---
lang: ur
direction: rtl
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-04T11:42:43.398592+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android ریلیز چیک لسٹ (and6)

اس چیک لسٹ نے ** اور 6 - CI اور تعمیل سختی ** گیٹس سے حاصل کیا ہے
`roadmap.md` (§ priority 5)۔ یہ Android SDK کو زنگ کے ساتھ ریلیز کرتا ہے
آر ایف سی کی توقعات کو سی آئی کی ملازمتوں ، تعمیل کے نوادرات کی ہجے کرکے جاری کریں ،
ڈیوائس لیب کے ثبوت ، اور پروویژن بنڈل جو GA سے پہلے منسلک ہونا ضروری ہے ،
ایل ٹی ایس ، یا ہاٹ فکس ٹرین آگے بڑھتی ہے۔

اس دستاویز کے ساتھ مل کر استعمال کریں:

- `docs/source/android_support_playbook.md` - ریلیز کیلنڈر ، SLAs ، اور
  بڑھتی ہوئی درخت
- `docs/source/android_runbook.md` - دن - سے دن آپریشنل رن بوکس۔
- `docs/source/compliance/android/and6_compliance_checklist.md` - ریگولیٹر
  آرٹ فیکٹ انوینٹری۔
- `docs/source/release_dual_track_runbook.md`- دوہری ٹریک کی رہائی کی حکمرانی۔

## 1۔ ایک نظر میں اسٹیج گیٹس

| اسٹیج | مطلوبہ دروازے | ثبوت |
| ------- | ---------------- | ---------- |
| ** t-7 دن (پری فریز) ** | رات کے وقت `ci/run_android_tests.sh` گرین 14 دن کے لئے ؛ `ci/check_android_fixtures.sh` ، `ci/check_android_samples.sh` ، اور `ci/check_android_docs_i18n.sh` پاسنگ ؛ لنٹ/انحصار اسکین قطار میں لگے۔ | بلڈکائٹ ڈیش بورڈز ، حقیقت میں مختلف رپورٹ ، نمونہ اسکرین شاٹ اسنیپ شاٹس۔ |
| ** ٹی - 3 دن (آر سی پروموشن) ** | ڈیوائس لیب ریزرویشن کی تصدیق ؛ مضبوط باکس کی تصدیق CI رن (`scripts/android_strongbox_attestation_ci.sh`) ؛ شیڈول ہارڈ ویئر پر استعمال شدہ روبوولیٹرک/انسٹرومینٹ سوٹ۔ `./gradlew lintRelease ktlintCheck detekt dependencyGuard` صاف۔ | ڈیوائس میٹرکس CSV ، تصدیق کا بنڈل مینی فیسٹ ، گریڈ کی رپورٹیں `artifacts/android/lint/<version>/` کے تحت محفوظ شدہ دستاویزات۔ |
| ** t-1 دن (go/no-go) ** | ٹیلی میٹری ریڈیکشن اسٹیٹس بنڈل ریفریشڈ (`scripts/telemetry/check_redaction_status.py --write-cache`) ؛ تعمیل نوادرات میں تازہ ترین `and6_compliance_checklist.md` ؛ پروویژن ریہرسل مکمل (`scripts/android_sbom_provenance.sh --dry-run`)۔ | `docs/source/compliance/android/evidence_log.csv` ، ٹیلی میٹری کی حیثیت JSON ، پروویژن ڈرائی رن لاگ۔ |
| ** T0 (GA/LTS کٹ اوور) ** | `scripts/publish_android_sdk.sh --dry-run` مکمل ؛ پروویژن + ایس بی او ایم پر دستخط ؛ ریلیز چیک لسٹ برآمد شدہ اور جانے کے لئے/نو گو منٹ کے لئے منسلک ؛ `ci/sdk_sorafs_orchestrator.sh` دھواں نوکری سبز۔ | `artifacts/android/` کے تحت آر ایف سی منسلکات ، Sigstore بنڈل ، گود لینے کے نمونے جاری کریں۔ |
| ** ٹی+1 دن (بعد کے بعد) ** | ہاٹ فکس کی تیاری کی تصدیق (`scripts/publish_android_sdk.sh --validate-bundle`) ؛ ڈیش بورڈ میں فرق جائزہ لیا گیا (`ci/check_android_dashboard_parity.sh`) ؛ `status.md` پر اپ لوڈ کردہ ثبوت پیکٹ۔ | ڈیش بورڈ ڈف برآمد ، `status.md` انٹری سے لنک ، آرکائیو ریلیز پیکٹ۔ |

## 2. CI اور کوالٹی گیٹ میٹرکس| گیٹ | کمانڈ (زبانیں) / اسکرپٹ | نوٹ |
| ------ | ---------------------- | ------- |
| یونٹ + انضمام ٹیسٹ | `ci/run_android_tests.sh` (لپیٹ `ci/run_android_tests.sh`) | `artifacts/android/tests/test-summary.json` + ٹیسٹ لاگ کو خارج کرتا ہے۔ Norito کوڈیک ، قطار ، مضبوط باکس فال بیک ، اور Torii کلائنٹ کنٹرول ٹیسٹ شامل ہیں۔ رات کو اور ٹیگنگ سے پہلے ضروری ہے۔ |
| حقیقت کی برابری | `ci/check_android_fixtures.sh` (لپیٹ `scripts/check_android_fixtures.py`) | اس بات کو یقینی بناتا ہے کہ دوبارہ تخلیق شدہ Norito فکسچر مورچا کیننیکل سیٹ سے ملتے ہیں۔ جب گیٹ ناکام ہوجاتا ہے تو JSON مختلف کو جوڑیں۔ |
| نمونہ ایپس | `ci/check_android_samples.sh` | `examples/android/{operator-console,retail-wallet}` بناتا ہے اور `scripts/android_sample_localization.py` کے ذریعے مقامی اسکرین شاٹس کی توثیق کرتا ہے۔ |
| دستاویزات/i18n | `ci/check_android_docs_i18n.sh` | گارڈز ریڈم + لوکلائزڈ کوئیک اسٹارٹ۔ ریلیز برانچ میں ڈاکٹر میں ترمیم کرنے کے بعد دوبارہ چلائیں۔ |
| ڈیش بورڈ برابری | `ci/check_android_dashboard_parity.sh` | تصدیق کرتا ہے CI/برآمد شدہ میٹرکس مورچا ہم منصبوں کے ساتھ سیدھ میں ہیں۔ T+1 کی توثیق کے دوران ضروری ہے۔ |
| SDK اپنانے والا دھواں | `ci/sdk_sorafs_orchestrator.sh` | موجودہ SDK کے ساتھ ملٹی سورس سورافس آرکسٹریٹر بائنڈنگ کی مشق کرتا ہے۔ اسٹیجڈ نوادرات کو اپ لوڈ کرنے سے پہلے ضروری ہے۔ |
| تصدیق کی توثیق | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | `artifacts/android/attestation/**` کے تحت مضبوط باکس/ٹی کی تصدیق کے بنڈل کو جمع کرتا ہے۔ GA پیکٹوں سے خلاصہ منسلک کریں۔ |
| ڈیوائس لیب سلاٹ توثیق | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | پیکٹوں کو جاری کرنے کے لئے ثبوت منسلک کرنے سے پہلے آلہ سازی کے بنڈل کی توثیق کرتا ہے۔ CI `fixtures/android/device_lab/slot-sample` (ٹیلی میٹری/تصدیق/قطار/لاگز + `sha256sum.txt`) میں نمونہ سلاٹ کے خلاف چلتا ہے۔ |

> ** اشارہ: ** ان ملازمتوں کو `android-release` بلڈکائٹ پائپ لائن میں شامل کریں تاکہ
> ہفتوں کو منجمد کرنے والے ہفتوں کو ریلیز برانچ کے اشارے کے ساتھ ہر گیٹ کو خود بخود دوبارہ چلائیں۔

مستحکم `.github/workflows/android-and6.yml` ملازمت لنٹ چلاتی ہے ،
ٹیسٹ سوٹ ، تصدیق شدہ سمریری ، اور ہر PR/پش پر ڈیوائس لیب سلاٹ چیک
`artifacts/android/{lint,tests,attestation,device_lab}/` کے تحت Android ذرائع کو چھونے ، شواہد اپ لوڈ کرنا۔

## 3. لنٹ اور انحصار اسکین

ریپو روٹ سے `scripts/android_lint_checks.sh --version <semver>` چلائیں۔
اسکرپٹ پر عملدرآمد:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- رپورٹس اور انحصار گارڈ آؤٹ پٹ کے تحت محفوظ شدہ دستاویزات ہیں
  `artifacts/android/lint/<label>/` اور رہائی کے لئے ایک `latest/` Symlink
  پائپ لائنز
- لنٹ کے نتائج کو ناکام بنانے کے لئے یا تو تدارک یا رہائی میں اندراج کی ضرورت ہوتی ہے
  آر ایف سی قبول شدہ رسک کی دستاویزات (ریلیز انجینئرنگ + پروگرام کے ذریعہ منظور شدہ
  لیڈ)۔
- `dependencyGuardBaseline` انحصار لاک کو دوبارہ تخلیق کرتا ہے۔ فرق جوڑیں
  جانے کے لئے/نو گو پیکٹ۔

## 4. ڈیوائس لیب اور مضبوط باکس کوریج

1. ریزرو پکسل + گلیکسی ڈیوائسز جس میں گنجائش والے ٹریکر کا حوالہ دیا گیا ہے
   `docs/source/compliance/android/device_lab_contingency.md`۔ بلاکس ریلیز
   اگر  cet تصدیق کی رپورٹ کو تازہ دم کرنے کے لئے۔
3. انسٹرومینٹیشن میٹرکس چلائیں (ڈیوائس میں سویٹ/ABI کی فہرست دستاویز کریں
   ٹریکر)۔ واقعات لاگ میں ناکامیوں پر قبضہ کریں یہاں تک کہ اگر کوششیں کامیاب ہوں۔
4. اگر فائر بیس ٹیسٹ لیب میں فال بیک بیک کی ضرورت ہو تو ٹکٹ فائل کریں۔ ٹکٹ لنک کریں
   ذیل میں چیک لسٹ میں۔

## 5. تعمیل اور ٹیلی میٹری نوادرات- EU کے لئے `docs/source/compliance/android/and6_compliance_checklist.md` پر عمل کریں
  اور جے پی گذارشات۔ `docs/source/compliance/android/evidence_log.csv` کو اپ ڈیٹ کریں
  ہیش + بلڈکائٹ جاب یو آر ایل کے ساتھ۔
- ٹیلی میٹری ریڈیکشن شواہد کو ریفریش کریں
  `اسکرپٹ/ٹیلی میٹری/چیک_ریڈیشن_سٹاٹس.پی-لکھنا-کیش \
   -اسٹیٹس-یو آر ایل https://android-observability.example/status.json`۔
  نتیجے میں JSON کے تحت اسٹور کریں
  `artifacts/android/telemetry/<version>/status.json`۔
- اسکیما ڈف آؤٹ پٹ ریکارڈ کریں
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  مورچا برآمد کنندگان کے ساتھ برابری ثابت کرنا۔

## 6. پروویژن ، ایس بی او ایم ، اور اشاعت

1. خشک کرنے والی پائپ لائن:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. SBOM + Sigstore پروویژن تیار کریں:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. `artifacts/android/provenance/<semver>/manifest.json` منسلک کریں اور دستخط شدہ
   `checksums.sha256` ریلیز RFC کے لئے۔
4. جب اصلی ماون ریپوزٹری کو فروغ دیتے ہو ، دوبارہ
   `scripts/publish_android_sdk.sh` بغیر `--dry-run` ، کنسول پر قبضہ کریں
   لاگ ان کریں ، اور نتیجے میں نمونے کو `artifacts/android/maven/<semver>` پر اپ لوڈ کریں۔

## 7. جمع کرانے والے پیکٹ ٹیمپلیٹ

ہر GA/LTS/ہاٹ فکس ریلیز میں شامل ہونا چاہئے:

1. ** مکمل شدہ چیک لسٹ ** - اس فائل کے ٹیبل کو کاپی کریں ، ہر آئٹم پر نشان لگائیں ، اور لنک
   نوادرات کی حمایت کرنے کے لئے (بلڈکائٹ رن ، لاگ ، ڈاکٹر ڈفنس)۔
2. ** ڈیوائس لیب ثبوت ** - تصدیق کی رپورٹ کا خلاصہ ، ریزرویشن لاگ ، اور
   کوئی بھی ہنگامی سرگرمیاں۔
3.
   `docs/source/sdk/android/telemetry_redaction.md` اپڈیٹس (اگر کوئی ہے)۔
4. ** تعمیل کے نوادرات ** - تعمیل فولڈر میں اندراجات شامل/تازہ کاری
   اس کے علاوہ تازہ دم ثبوت لاگ CSV۔
5.
6.
   مذکورہ بالا (تاریخ ، ورژن ، کسی چھوٹے ہوئے دروازوں کی خاص بات)۔

پیکٹ کو `artifacts/android/releases/<version>/` کے تحت اسٹور کریں اور اس کا حوالہ دیں
`status.md` اور ریلیز RFC میں۔

- `scripts/run_release_pipeline.py --publish-android-sdk ...` خود بخود
  تازہ ترین لنٹ آرکائیو (`artifacts/android/lint/latest`) اور دی کاپی کرتا ہے
  تعمیل ثبوت `artifacts/android/releases/<version>/` میں لاگ ان کریں
  جمع کرانے والے پیکٹ میں ہمیشہ ایک اہم مقام ہوتا ہے۔

---

** یاد دہانی: ** جب بھی نئی سی آئی ملازمتیں ، تعمیل کے نوادرات ، اس چیک لسٹ کو اپ ڈیٹ کریں ،
یا ٹیلی میٹری کی ضروریات کو شامل کیا جاتا ہے۔ روڈ میپ آئٹم اور 6 اس وقت تک کھلا رہتا ہے
چیک لسٹ اور اس سے وابستہ آٹومیشن مسلسل دو رہائی کے لئے مستحکم ثابت ہوں
ٹرینیں