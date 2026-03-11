---
lang: ur
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2026-01-03T18:07:58.297179+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: staging-manifest-playbook-ur
slug: /sorafs/staging-manifest-playbook-ur
---

::: نوٹ کینونیکل ماخذ
آئینہ `docs/source/sorafs/runbooks/staging_manifest_playbook.md`۔ دونوں کاپیاں ریلیز کے پار منسلک رکھیں۔
:::

## جائزہ

یہ پلے بوک پروڈکشن میں تبدیلی کو فروغ دینے سے پہلے ایک اسٹیجنگ Torii تعیناتی پر پارلیمنٹ سے متعلق چنکر پروفائل کو چالو کرنے کے ذریعے چلتی ہے۔ اس نے فرض کیا ہے کہ SoraFS گورننس چارٹر کی توثیق کی گئی ہے اور کیننیکل فکسچر ریپوزٹری میں دستیاب ہیں۔

## 1. شرائط

1. کیننیکل فکسچر اور دستخطوں کو ہم آہنگ کریں:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. داخلہ لفافہ ڈائرکٹری تیار کریں جو Torii اسٹارٹ اپ (مثال کے طور پر راستہ) پر پڑھے گا: `/var/lib/iroha/admission/sorafs`۔
3. یقینی بنائیں کہ Torii کنفیگ دریافت کیش اور داخلے کے نفاذ کو قابل بناتا ہے:

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

## 2۔ داخلہ لفافے شائع کریں

1. `torii.sorafs.discovery.admission.envelopes_dir` کے ذریعہ حوالہ کردہ ڈائریکٹری میں منظور شدہ فراہم کنندہ داخلہ لفافوں کاپی کریں:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Torii کو دوبارہ شروع کریں (یا اگر آپ نے فلائی دوبارہ لوڈ کے ساتھ لوڈر کو لپیٹا ہے تو ایک سگ اپ بھیجیں)۔
3. داخلے کے پیغامات کے لئے لاگ کو دم کریں:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. دریافت کی تشہیر کی توثیق کریں

1. دستخط شدہ فراہم کنندہ کے اشتہار پے لوڈ (Norito بائٹس) کو آپ کے ذریعہ تیار کریں
   فراہم کنندہ پائپ لائن:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. دریافت کے اختتامی نقطہ سے استفسار کریں اور تصدیق کریں کہ اشتہار کیننیکل عرفی ناموں کے ساتھ ظاہر ہوتا ہے:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   یقینی بنائیں کہ `profile_aliases` میں پہلی اندراج کے طور پر `"sorafs.sf1@1.0.0"` شامل ہے۔

## 4. ورزش مینی فیسٹ اور پلان اینڈ پوائنٹس

1. مینی فیسٹ میٹا ڈیٹا کو بازیافت کریں (اگر داخلہ نافذ کیا جاتا ہے تو ایک اسٹریم ٹوکن کی ضرورت ہوتی ہے):

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
   - `manifest_digest_hex` عزم کی رپورٹ سے میل کھاتا ہے۔
   - `chunk_digests_blake3` دوبارہ پیدا ہونے والے فکسچر کے ساتھ سیدھ میں ہے۔

## 5. ٹیلی میٹری چیک

- تصدیق کریں Prometheus نئے پروفائل میٹرکس کو بے نقاب کرتا ہے:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- ڈیش بورڈز کو متوقع عرف کے تحت اسٹیجنگ فراہم کرنے والے کو دکھانا چاہئے اور براؤن آؤٹ کاؤنٹرز کو صفر پر رکھنا چاہئے جبکہ پروفائل فعال ہے۔

## 6. رول آؤٹ تیاری

1. یو آر ایل ، مینی فیسٹ آئی ڈی ، اور ٹیلی میٹری اسنیپ شاٹ کے ساتھ ایک مختصر رپورٹ پر قبضہ کریں۔
2. منصوبہ بند پروڈکشن ایکٹیویشن ونڈو کے ساتھ ساتھ Nexus رول آؤٹ چینل میں رپورٹ شیئر کریں۔
3. اسٹیک ہولڈرز کے دستخط ہونے کے بعد پروڈکشن چیک لسٹ (`chunker_registry_rollout_checklist.md` میں سیکشن 4) پر آگے بڑھیں۔

اس پلے بوک کو اپ ڈیٹ رکھنا ہر چنکر/داخلہ رول آؤٹ کو یقینی بناتا ہے کہ اسٹیجنگ اور پروڈکشن میں ایک ہی عصبی اقدامات کی پیروی کی جائے۔
