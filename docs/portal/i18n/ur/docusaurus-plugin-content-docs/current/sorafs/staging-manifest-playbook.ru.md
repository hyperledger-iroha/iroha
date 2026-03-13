---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: اسٹیجنگ مائی فیسٹ پلے بوک
عنوان: اسٹیجنگ کے لئے مینی فیسٹ پلے بک
سائڈبار_لیبل: اسٹیجنگ کے لئے مینی فیسٹ پلے بک
تفصیل: اسٹیجنگ تعیناتیوں Torii پر پارلیمنٹ سے متعلق چنکر پروفائل کو چالو کرنے کے لئے چیک لسٹ۔
---

::: نوٹ کینونیکل ماخذ
:::

## جائزہ

اس پلے بوک نے پروڈکشن میں تبدیلیوں کو آگے بڑھانے سے پہلے اسٹیجنگ تعیناتی Torii پر پارلیمنٹ سے متعلق چنکر پروفائل کو قابل بنانے کی وضاحت کی ہے۔ یہ فرض کیا جاتا ہے کہ گورننس چارٹر SoraFS کی توثیق کی گئی ہے اور کیننیکل فکسچر ریپوزٹری میں دستیاب ہیں۔

## 1. شرائط

1. کنوینیکل فکسچر اور کیپشن کو ہم آہنگ کریں:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. داخلہ لفافے ڈائریکٹری تیار کریں ، جو Torii اسٹارٹ اپ (مثال کے طور پر راستہ) پر پڑھتا ہے: `/var/lib/iroha/admission/sorafs`۔
3. اس بات کو یقینی بنائیں کہ Torii تشکیل میں دریافت کیشے اور نفاذ کا داخلہ شامل ہے:

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

## 2۔ داخلہ لفافوں کی اشاعت

1. `torii.sorafs.discovery.admission.envelopes_dir` میں مخصوص ڈائریکٹری میں منظور شدہ فراہم کنندہ داخلہ لفافوں کاپی کریں:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii کو دوبارہ شروع کریں (یا اگر آپ نے بوٹ لوڈر ہاٹ ریلوڈ لپیٹ لیا ہے تو SIGHUP بھیجیں)۔
3. داخلے کے نوشتہ جات کی نگرانی کریں:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. دریافت کی تقسیم کی جانچ کرنا

1. اپنے فراہم کنندہ پائپ لائن کے ذریعہ تیار کردہ دستخط شدہ فراہم کنندہ کے اشتہار پے لوڈ (Norito بائٹس) کو شائع کریں:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v2/sorafs/provider/advert
   ```

2. اختتامی نقطہ دریافت کی درخواست کریں اور یقینی بنائیں کہ اشتہار عرفی ناموں کے ساتھ دکھایا گیا ہے:

   ```bash
   curl -sS http://staging-torii:8080/v2/sorafs/providers | jq .
   ```

   اس بات کو یقینی بنائیں کہ `profile_aliases` میں پہلے عنصر کے طور پر `"sorafs.sf1@1.0.0"` پر مشتمل ہے۔

## 4. اختتامی مقامات کی جانچ پڑتال اور منصوبہ بندی

1. میٹا ڈیٹا مینی فیسٹ حاصل کریں (اگر داخلہ نافذ کیا گیا ہو تو اسٹریم ٹوکن کی ضرورت ہے):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON آؤٹ پٹ کو چیک کریں اور اس بات کو یقینی بنائیں کہ:
   - `chunk_profile_handle` `sorafs.sf1@1.0.0` کے برابر ہے۔
   - `manifest_digest_hex` عزم کی رپورٹ سے میل کھاتا ہے۔
   - `chunk_digests_blake3` دوبارہ پیدا ہونے والے فکسچر سے ملتے ہیں۔

## 5. ٹیلی میٹری چیک

- تصدیق کریں کہ Prometheus نئے پروفائل میٹرکس کو شائع کرتا ہے:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ڈیش بورڈز کو متوقع عرف کے تحت اسٹیجنگ فراہم کرنے والے کو دکھانا چاہئے اور براؤن آؤٹ کاؤنٹرز کو صفر پر رکھنا چاہئے جبکہ پروفائل فعال ہے۔

## 6. رول آؤٹ کے لئے تیار ہے

1. یو آر ایل ، آئی ڈی مینی فیسٹ اور ٹیلی میٹری اسنیپ شاٹ کے ساتھ ایک مختصر رپورٹ تیار کریں۔
2. پیداوار میں شیڈول ایکٹیویشن ونڈو کے ساتھ Nexus رول آؤٹ چینل میں رپورٹ شیئر کریں۔
3. اسٹیک ہولڈرز کے ساتھ معاہدے کے بعد پروڈکشن چیک لسٹ (`chunker_registry_rollout_checklist.md` میں سیکشن 4) پر آگے بڑھیں۔

اس پلے بوک کو تازہ ترین رکھنا اس بات کو یقینی بناتا ہے کہ ہر رول آؤٹ چنکر/داخلہ اسٹیجنگ اور پروڈکشن کے مابین ایک ہی عصبی اقدامات کی پیروی کرتا ہے۔