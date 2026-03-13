---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: اسٹیجنگ مائی فیسٹ پلے بوک
عنوان: اسٹیجنگ میں مینی فیسٹ پلے بک
سائڈبار_لیبل: اسٹیجنگ میں مینی فیسٹ پلے بک
تفصیل: Torii تعیناتیوں میں پارلیمنٹ کے ذریعہ تصدیق شدہ چنکر پروفائل کو قابل بنانے کے لئے چیک لسٹ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/staging_manifest_playbook.md` کا آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

## جائزہ

اس پلے بوک میں بتایا گیا ہے کہ پروڈکشن میں جانے سے پہلے Torii تعیناتی پر پارلیمنٹ سے متعلق چنکر پروفائل کو کس طرح قابل بنایا جائے۔ یہ فرض کرتا ہے کہ SoraFS گورننس چارٹر کی توثیق کی گئی ہے اور یہ کہ کیننیکل فکسچر ریپوزٹری میں دستیاب ہیں۔

## 1. شرائط

1. کنوینیکل فکسچر اور دستخطوں کو ہم آہنگ کریں:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. داخلہ لفافے ڈائریکٹری تیار کریں جو Torii اسٹارٹ اپ (مثال کے طور پر راستہ) پر پڑھے گا: `/var/lib/iroha/admission/sorafs`۔
3. اس بات کو یقینی بنائیں کہ Torii کنفیگ دریافت کیشے اور داخلے کے نفاذ کو قابل بناتا ہے:

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

1. `torii.sorafs.discovery.admission.envelopes_dir` کے ذریعہ حوالہ کردہ ڈائریکٹری میں منظور شدہ فراہم کنندہ داخلہ لفافوں کاپی کریں:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii کو دوبارہ شروع کریں (یا اگر آپ نے لوڈر کو دوبارہ لوڈ کیا تو ایک سگ اپ بھیجیں)۔
3. داخلہ پیغامات کے لئے لاگ ان کی نگرانی کریں:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. دریافت کی تشہیر کی توثیق کریں

1. فراہم کنندہ پائپ لائن کے ذریعہ تیار کردہ دستخط شدہ فراہم کنندہ کے اشتہار پے لوڈ (بائٹس Norito) کو شائع کریں:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. دریافت کے اختتامی نقطہ کی جانچ کریں اور تصدیق کریں کہ اشتہار کیننیکل عرفی ناموں کے ساتھ ظاہر ہوتا ہے:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   اس بات کو یقینی بنائیں کہ `profile_aliases` میں پہلی اندراج کے طور پر `"sorafs.sf1@1.0.0"` شامل ہے۔

## 4. ورزش مینی فیسٹ اور منصوبہ بندی کے اختتامی نکات

1. مینی فیسٹ میٹا ڈیٹا تلاش کریں (اگر داخلہ نافذ کیا گیا ہو تو اسٹریم ٹوکن کی ضرورت ہوتی ہے):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON کا معائنہ کریں اور چیک کریں:
   - `chunk_profile_handle` اور `sorafs.sf1@1.0.0`۔
   - `manifest_digest_hex` عزم کی رپورٹ سے مطابقت رکھتا ہے۔
   - `chunk_digests_blake3` دوبارہ پیدا ہونے والے فکسچر کے ساتھ سیدھ میں ہے۔

## 5. ٹیلی میٹری چیک

- تصدیق کریں کہ Prometheus نئے پروفائل میٹرکس کو بے نقاب کرتا ہے:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ڈیش بورڈز کو متوقع عرف کے تحت اسٹیجنگ فراہم کرنے والے کو دکھانا چاہئے اور براؤن آؤٹ کاؤنٹرز کو صفر پر رکھنا چاہئے جبکہ پروفائل فعال ہے۔

## 6. رول آؤٹ تیاری

1. یو آر ایل ، مینی فیسٹ آئی ڈی اور ٹیلی میٹری اسنیپ شاٹ کے ساتھ ایک مختصر رپورٹ پر قبضہ کریں۔
2. Nexus رول آؤٹ چینل میں رپورٹ کو منصوبہ بند پروڈکشن گو لائو ونڈو کے ساتھ شیئر کریں۔
3. پروڈکشن چیک لسٹ میں آگے بڑھیں (`chunker_registry_rollout_checklist.md` میں سیکشن 4) جب اسٹیک ہولڈرز کی منظوری دی جائے۔

اس پلے بوک کو اپ ڈیٹ رکھتے ہوئے اس بات کو یقینی بناتا ہے کہ ہر چنکر/داخلہ رول آؤٹ اسٹیجنگ اور پروڈکشن کے مابین ایک ہی عصبی اقدامات کی پیروی کرتا ہے۔