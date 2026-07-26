---
lang: ur
direction: rtl
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-04T11:42:43.493867+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM اور پروویژن تصدیق - Android SDK

| فیلڈ | قیمت |
| ------- | ------- |
| دائرہ کار | Android SDK (`java/iroha_android`) + نمونہ ایپس (`examples/android/*`) |
| ورک فلو مالک | ریلیز انجینئرنگ (الیکسی موروزوف) |
| آخری تصدیق شدہ | 2026-02-11 (بلڈکائٹ `android-sdk-release#4821`) |

## 1. جنریشن ورک فلو

ہیلپر اسکرپٹ چلائیں (اور 6 آٹومیشن کے لئے شامل کیا گیا ہے):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

اسکرپٹ مندرجہ ذیل کام انجام دیتا ہے:

1. `ci/run_android_tests.sh` اور `scripts/check_android_samples.sh` پر عملدرآمد کرتا ہے۔
2. `examples/android/` کے تحت گریڈ ریپر کی درخواست کرتا ہے
   `:android-sdk` ، `:operator-console` ، اور `:retail-wallet` فراہم کردہ کے ساتھ
   `-PversionName`۔
3. ہر SBOM کو `artifacts/android/sbom/<sdk-version>/` میں کیننیکل ناموں کے ساتھ کاپی کرتا ہے
   (`iroha-android.cyclonedx.json` ، وغیرہ)۔

## 2. پروویژن اینڈ سائننگ

وہی اسکرپٹ `cosign sign-blob --bundle <file>.sigstore --yes` کے ساتھ ہر SBOM کی علامت ہے
اور منزل کی ڈائرکٹری میں `checksums.txt` (SHA-256) خارج کرتا ہے۔ `COSIGN` سیٹ کریں
ماحولیاتی متغیر اگر بائنری `$PATH` سے باہر رہتی ہے۔ اسکرپٹ ختم ہونے کے بعد ،
بنڈل/چیکسم کے راستے کے علاوہ بلڈکائٹ رن ID ریکارڈ کریں
`docs/source/compliance/android/evidence_log.csv`۔

## 3. تصدیق

شائع شدہ ایس بی او ایم کی تصدیق کے لئے:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

آؤٹ پٹ SHA کا موازنہ `checksums.txt` میں درج قدر سے کریں۔ جائزہ لینے والے پچھلے ریلیز کے خلاف ایس بی او ایم کو بھی مختلف کرتے ہیں تاکہ یہ یقینی بنایا جاسکے کہ انحصار ڈیلٹا جان بوجھ کر ہے۔

## 4. ثبوت سنیپ شاٹ (2026-02-11)

| اجزاء | sbom | SHA-256 | Sigstore بنڈل |
| ----------- | ------ | --------- | ------------------- |
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` بنڈل SBOM کے ساتھ محفوظ ہے |
| آپریٹر کنسول نمونہ | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| پرچون پرس کا نمونہ | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*۔

## 5. بقایا کام

- GA سے پہلے ریلیز پائپ لائن کے اندر SBOM + Cosign اقدامات کو خودکار کریں۔
- ایک بار عوامی نمونے والی بالٹی کے لئے آئینہ ایس بی او ایم ایس اور چیک لسٹ کو مکمل طور پر 6 نمبر پر۔
- ایس بی او ایم ڈاؤن لوڈ کے مقامات کو پارٹنر کا سامنا کرنے والے ریلیز نوٹوں سے لنک کرنے کے لئے دستاویزات کے ساتھ مربوط کریں۔