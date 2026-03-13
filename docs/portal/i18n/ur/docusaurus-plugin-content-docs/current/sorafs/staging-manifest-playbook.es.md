---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: اسٹیجنگ مائی فیسٹ پلے بوک
عنوان: اسٹیجنگ میں منشور کی پلے بک
سائڈبار_لیبل: اسٹیجنگ میں مینی فیسٹ پلے بک
تفصیل: Torii اسٹیجنگ تعیناتیوں میں پارلیمنٹ سے متعلق چنکر پروفائل کو قابل بنانے کے لئے چیک لسٹ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/staging_manifest_playbook.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

## خلاصہ

اس پلے بوک میں بتایا گیا ہے کہ کس طرح پروڈکشن میں سوئچ کو فروغ دینے سے پہلے ایک اسٹیجنگ Torii تعیناتی پر پارلیمنٹ سے متعلق چنکر پروفائل کو قابل بنایا جائے۔ فرض کریں کہ SoraFS کے گورننس چارٹر کی توثیق کی گئی ہے اور یہ کہ کیننیکل فکسچر ریپوزٹری میں دستیاب ہیں۔

## 1. شرائط

1. کنوینیکل فکسچر اور دستخطوں کو ہم آہنگ کریں:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. داخلہ لفافہ ڈائرکٹری تیار کریں جو Torii اسٹارٹ اپ (مثال کے طور پر راستہ) پر پڑھے گا: `/var/lib/iroha/admission/sorafs`۔
3. اس بات کو یقینی بنائیں کہ Torii ترتیب دریافت کیشے اور درخواست میں داخلے کے قابل بنائے:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2۔ داخلہ لفافے پوسٹ کریں

1. `torii.sorafs.discovery.admission.envelopes_dir` کے ذریعہ حوالہ کردہ ڈائریکٹری میں منظور شدہ داخلہ لفافوں کاپی کریں:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. ربوٹ Torii (یا اگر آپ نے لوڈر کو گرم دوبارہ لوڈ سے لپیٹا ہے تو ایک سگ اپ بھیجیں)۔
3. داخلہ پیغامات کے لئے لاگ ان چیک کریں:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. دریافت کی تشہیر کی توثیق کریں

1. دستخط شدہ فراہم کنندہ کے اشتہار پے لوڈ (بائٹس Norito) کو اپنے فراہم کنندہ پائپ لائن کے ذریعہ تیار کردہ شائع کریں:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. دریافت کے اختتامی نقطہ کی جانچ کریں اور تصدیق کریں کہ اشتہار کیننیکل عرفی ناموں کے ساتھ ظاہر ہوتا ہے:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   اس بات کو یقینی بناتا ہے کہ `profile_aliases` میں `"sorafs.sf1@1.0.0"` کو پہلی اندراج کے طور پر شامل کیا گیا ہے۔

## 4۔ ٹیسٹ مینی فیسٹ اور پلان اینڈ پوائنٹس

1. مینی فیسٹ میٹا ڈیٹا حاصل کریں (اگر داخلہ فعال ہے تو اسٹریم ٹوکن کی ضرورت ہے):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON آؤٹ پٹ کا معائنہ کریں اور تصدیق کریں:
   - `chunk_profile_handle` `sorafs.sf1@1.0.0` ہے۔
   - `manifest_digest_hex` عزم کی رپورٹ سے مماثل ہے۔
   - `chunk_digests_blake3` دوبارہ پیدا ہونے والے فکسچر کے ساتھ سیدھ میں ہے۔

## 5. ٹیلی میٹری چیک

- تصدیق کریں کہ Prometheus نئے پروفائل میٹرکس کو بے نقاب کرتا ہے:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ڈیش بورڈز کو متوقع عرف کے تحت اسٹیجنگ فراہم کرنے والے کو دکھانا چاہئے اور براؤن آؤٹ کاؤنٹرز کو صفر پر رکھنا چاہئے جبکہ پروفائل فعال ہے۔

## 6. رول آؤٹ کی تیاری

1. یو آر ایل ، مینی فیسٹ آئی ڈی اور ٹیلی میٹری اسنیپ شاٹ کے ساتھ ایک مختصر رپورٹ پر قبضہ کریں۔
2. منصوبہ بند پروڈکشن ایکٹیویشن ونڈو کے ساتھ Nexus رول آؤٹ چینل میں رپورٹ شیئر کریں۔
3. ایک بار دلچسپی رکھنے والی جماعتوں کو آگے بڑھنے کے بعد پروڈکشن چیک لسٹ (`chunker_registry_rollout_checklist.md` میں سیکشن 4) کے ساتھ جاری رکھیں۔

اس پلے بوک کو تازہ ترین رکھنا اس بات کو یقینی بناتا ہے کہ ہر چنکر/داخلہ رول آؤٹ اسٹیجنگ اور پروڈکشن کے مابین ایک ہی اختیاری اقدامات کی پیروی کرتا ہے۔