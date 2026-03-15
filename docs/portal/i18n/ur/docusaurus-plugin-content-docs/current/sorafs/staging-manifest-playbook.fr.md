---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: اسٹیجنگ مائی فیسٹ پلے بوک
عنوان: اسٹیجنگ مینی فیسٹ پلے بک
سائڈبار_لیبل: مینی فیسٹ پلے بک اسٹیجنگ
تفصیل: اسٹیجنگ تعیناتی Torii پر پارلیمنٹ کے ذریعہ تصدیق شدہ چنکر پروفائل کو چالو کرنے کے لئے چیک لسٹ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/staging_manifest_playbook.md` کی عکاسی کرتا ہے۔ کاپی Docusaurus اور لیگیسی مارک ڈاون کو اس وقت تک رکھیں جب تک کہ اسفینکس سیٹ مکمل طور پر ریٹائر نہ ہوجائے۔
:::

## جائزہ

اس پلے بوک میں پیداوار میں تبدیلی کو فروغ دینے سے پہلے ایک اسٹیجنگ Torii تعیناتی پر پارلیمنٹ کے ذریعہ توثیق شدہ چنکر پروفائل کی چالو کرنے کی وضاحت کی گئی ہے۔ یہ فرض کرتا ہے کہ گورننس چارٹر SoraFS کی توثیق کی گئی ہے اور یہ کہ کیننیکل فکسچر ریپوزٹری میں دستیاب ہیں۔

## 1. شرائط

1. کنوینیکل فکسچر اور دستخطوں کو ہم آہنگ کریں:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. داخلہ لفافہ ڈائرکٹری تیار کریں جو Torii اسٹارٹ اپ (مثال کے طور پر راستہ) پر پڑھے گا: `/var/lib/iroha/admission/sorafs`۔
3. یقینی بنائیں کہ Torii ترتیب کیشے کی دریافت اور داخلے کے نفاذ کو قابل بناتا ہے:

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

2. Torii کو دوبارہ اسٹارٹ کریں (یا اگر آپ نے لوڈر کو گرم دوبارہ لوڈ سے انکپولیٹ کیا ہے تو ایک سگ اپ بھیجیں)۔
3. داخلہ پیغامات کے لئے لاگ ان پر عمل کریں:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. دریافت کی تشہیر کی توثیق کریں

1. اپنے سپلائر پائپ لائن کے ذریعہ تیار کردہ دستخط شدہ سپلائر ایڈورٹائزمنٹ پے لوڈ (بائٹس Norito) پوسٹ کریں:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. دریافت کے اختتامی نقطہ سے استفسار کریں اور تصدیق کریں کہ اشتہار کیننیکل عرفی ناموں کے ساتھ ظاہر ہوتا ہے:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   اس بات کو یقینی بنائیں کہ `profile_aliases` میں پہلی اندراج کے طور پر `"sorafs.sf1@1.0.0"` شامل ہے۔

## 4. ورزش مینی فیسٹ اور منصوبہ بندی کے اختتامی نکات

1. مینی فیسٹ میٹا ڈیٹا بازیافت کریں (اگر داخلہ لاگو ہوتا ہے تو اسٹریم ٹوکن کی ضرورت ہوتی ہے):

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
   - `manifest_digest_hex` عزم کی رپورٹ سے مطابقت رکھتا ہے۔
   - `chunk_digests_blake3` دوبارہ پیدا ہونے والے فکسچر کے ساتھ سیدھ میں ہے۔

## 5. ٹیلی میٹری چیک

- تصدیق کریں کہ Prometheus نئے پروفائل میٹرکس کو بے نقاب کرتا ہے:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ڈیش بورڈز کو متوقع عرف کے تحت اسٹیجنگ فراہم کرنے والے کو دکھانا چاہئے اور جب تک پروفائل فعال نہیں ہے اس وقت تک براؤن آؤٹ کاؤنٹرز کو صفر پر رکھنا چاہئے۔

## 6. رول آؤٹ کی تیاری

1. یو آر ایل ، مینی فیسٹ آئی ڈی اور ٹیلی میٹری اسنیپ شاٹ کے ساتھ ایک مختصر رپورٹ پر قبضہ کریں۔
2. رول آؤٹ چینل Nexus میں منصوبہ بند پروڈکشن ایکٹیویشن ونڈو کے ساتھ رپورٹ شیئر کریں۔
3. اسٹیک ہولڈرز کے اتفاق ہونے کے بعد پروڈکشن چیک لسٹ (`chunker_registry_rollout_checklist.md` میں سیکشن 4) پر آگے بڑھیں۔

اس پلے بوک کو تازہ ترین رکھنا اس بات کو یقینی بناتا ہے کہ ہر رول آؤٹ چنکر/داخلہ اسٹیجنگ اور پروڈکشن کے مابین ایک ہی عصبی اقدامات کی پیروی کرتا ہے۔