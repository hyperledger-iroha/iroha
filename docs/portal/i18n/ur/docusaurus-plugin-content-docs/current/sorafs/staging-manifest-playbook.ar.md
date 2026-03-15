---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/staging-manifest-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: اسٹیجنگ مائی فیسٹ پلے بوک
عنوان: اسٹیجنگ مینی فیسٹ گائیڈ
سائڈبار_لیبل: اسٹیجنگ مینی فیسٹ گائیڈ
تفصیل: Torii بلیٹن اسٹیجنگ پر پارلیمنٹری توثیق چنکر فائل کو چالو کرنے کے لئے چیک لسٹ۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/sorafs/runbooks/staging_manifest_playbook.md` کی عکاسی کرتا ہے۔ اس بات کو یقینی بنائیں کہ Docusaurus ورژن اور پرانے مارک ڈاون ورژن کو یکساں رکھیں جب تک کہ اسفینکس کلسٹر مکمل طور پر ریٹائر نہ ہوجائے۔
:::

## جائزہ

یہ گائیڈ آپ کو ہدایت دیتا ہے کہ آپ کو پیداوار میں تبدیلی کو فروغ دینے سے پہلے اسٹجنگ تعیناتی Torii میں پارلیمنٹ کی تصدیق شدہ چنکر فائل کو قابل بنائیں۔ یہ فرض کیا جاتا ہے کہ SoraFS گورننس چارٹر کو منظور کرلیا گیا ہے اور یہ منظور شدہ فکسچر ریپوزٹری کے اندر دستیاب ہیں۔

## 1. شرائط

1. منظور شدہ فکسچر اور دستخطوں کو ہم آہنگ کریں:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. قبولیت لفافہ ڈائرکٹری تیار کریں جو Torii بوٹ (مثال کے طور پر راستہ) پر پڑھتا ہے: `/var/lib/iroha/admission/sorafs`۔
3. اس بات کو یقینی بنائیں کہ Torii کی ترتیبات دریافت کیشے اور درخواست کی قبولیت کو قابل بنائیں:

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

## 2. پوسٹ قبولیت لفافے

1. `torii.sorafs.discovery.admission.envelopes_dir` میں اشارہ کردہ ڈائریکٹری میں منظور شدہ فراہم کنندہ قبولیت کے لفافوں کاپی کریں:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. ربوٹ Torii (یا اگر لوڈر فوری طور پر دوبارہ لوڈنگ کی حمایت کرتا ہے تو SIGHUP بھیجیں)۔
3. قبولیت کے خطوط کے لئے ریکارڈز کی نگرانی کریں:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. دریافت کے پھیلاؤ کی تصدیق کریں

1. فراہم کنندہ لائن کے ذریعہ تیار کردہ دستخط شدہ فراہم کنندہ کے اشتہار پے لوڈ (Norito بائٹس) کو شائع کریں:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. ڈسکوری پوائنٹ سے استفسار کریں اور اس بات کو یقینی بنائیں کہ اشتہار منظور شدہ عرفی ناموں کے ساتھ ظاہر ہوتا ہے:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   اس بات کو یقینی بنائیں کہ `profile_aliases` میں پہلی اندراج کے طور پر `"sorafs.sf1@1.0.0"` شامل ہے۔

## 4۔ ٹیسٹ مینی فیسٹ اور پلان اینڈ پوائنٹس

1. بازیافت مینی فیسٹ میٹا ڈیٹا (اگر قبول ہو تو اسٹریم ٹوکن کی ضرورت ہوتی ہے):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. JSON آؤٹ پٹ کی جانچ کریں اور چیک کریں:
   - کہ `chunk_profile_handle` کے برابر `sorafs.sf1@1.0.0`۔
   - کہ `manifest_digest_hex` عین مطابق رپورٹ سے مماثل ہے۔
   - `chunk_digests_blake3` دوبارہ پیدا ہونے والے فکسچر سے میل کھاتا ہے۔

## 5. ٹیلی میٹک امتحانات

- اس بات کو یقینی بنائیں کہ Prometheus نئی فائل میٹرکس دکھاتا ہے:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- مانیٹرنگ پینلز کو متوقع عرف کے تحت اسٹیجنگ فراہم کنندہ کو ظاہر کرنا چاہئے اور فائل فعال ہونے پر براؤن آؤٹ کاؤنٹرز صفر پر رہنا چاہئے۔

## 6. لانچ کے لئے تیاری

1. ایک مختصر رپورٹ پر قبضہ کریں جس میں یو آر ایل ، مینی فیسٹ آئی ڈی ، اور ٹیلی میٹک اسنیپ شاٹ شامل ہیں۔
2. Nexus رول آؤٹ چینل میں رپورٹ کو ایکٹیویشن ونڈو کے ساتھ جو پروڈکشن میں منصوبہ بند کیا گیا ہے اس کا اشتراک کریں۔
3. اسٹیک ہولڈر کی منظوری کے بعد پروڈکشن چیک لسٹ (`chunker_registry_rollout_checklist.md` میں سیکشن 4) پر جائیں۔

اس گائیڈ کو تازہ ترین رکھنے سے یہ یقینی بنتا ہے کہ چنکر/داخلے کا ہر لانچ اسٹیجنگ اور پروڈکشن کے مابین ایک ہی عصبی اقدامات کی پیروی کرتا ہے۔