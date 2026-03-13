---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: چنکر رجسٹری-رول آؤٹ-چیک لسٹ
عنوان: سرفس کے لئے چنکر لاگ لانچ کرنے کے لئے چیک لسٹ
سائڈبار_لیبل: چنکر لانچ چیک لسٹ
تفصیل: چنکر لاگ اپ ڈیٹس کے لئے مرحلہ وار ریلیز کا منصوبہ۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
`docs/source/sorafs/chunker_registry_rollout_checklist.md` کی عکاسی کریں۔ اس بات کو یقینی بنائیں کہ اس وقت تک دونوں کاپیاں مطابقت پذیری میں رکھیں جب تک کہ پرانی اسفنکس دستاویزات کا سیٹ ریٹائر نہ ہو۔
:::

# غیر مقفل رجسٹر SoraFS کے لئے چیک لسٹ

یہ فہرست نئی چنکر فائل یا فراہم کنندہ کے قبولیت پیکیج کو اپ گریڈ کرنے کے لئے درکار اقدامات جمع کرتی ہے
گورننس چارٹر کی توثیق کرنے کے بعد جائزہ لینے کے مرحلے سے لے کر پیداوار تک۔

> ** دائرہ کار: ** ان تمام ورژن پر لاگو ہوتا ہے جو ترمیم کرتے ہیں
> `sorafs_manifest::chunker_registry` یا سپلائرز کے قبولیت لفافے یا فکسچر پیکیجز
> منظور شدہ (`fixtures/sorafs_chunker/*`)۔

## 1. پہلے سے لانچ کی توثیق

1. فکسچر کو دوبارہ تخلیق کریں اور عزم کی جانچ کریں:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. اس بات کو یقینی بنائیں کہ عزم کے فنگر پرنٹ میں موجود ہیں
   `docs/source/sorafs/reports/sf1_determinism.md` (یا متعلقہ فائل رپورٹ)
   دوبارہ پیدا ہونے والے نشانات سے میل کھاتا ہے۔
3. اس بات کو یقینی بنائیں کہ `sorafs_manifest::chunker_registry` کے ساتھ بنایا گیا ہے
   `ensure_charter_compliance()` کے ذریعے:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. پیکیج پروپوزل فائل کو اپ ڈیٹ کریں:
   -`docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` میں بورڈ منٹ میں داخل ہوں
   - تعصب کی رپورٹ

## 2۔ گورننس کی منظوری

1. ٹولنگ ورکنگ گروپ کی رپورٹ اور ڈائجسٹ نے یہ تجویز سورہ پارلیمنٹ کے انفراسٹرکچر پینل کو پیش کی۔
2. رضامندی کی تفصیلات ریکارڈ کریں
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`۔
3. فکسچر کے ساتھ ہی پارلیمنٹ کے ذریعہ دستخط شدہ لفافے کو پوسٹ کریں:
   `fixtures/sorafs_chunker/manifest_signatures.json`۔
4. تصدیق کریں کہ لفافہ گورننس بازیافت اسسٹنٹ کے ذریعہ قابل رسائی ہے:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. لانچ اسٹیجنگ

تفصیلی وضاحت کے لئے [اسٹیجنگ مینی فیسٹ گائیڈ] (./staging-manifest-playbook) کا حوالہ دیں۔

1. `torii.sorafs` کے لئے دریافت کے ساتھ Torii کو تعی .ن کریں اور قبولیت کے لئے انفورسمنٹ آن کیا گیا
   (`enforce_admission = true`)۔
2. منظور شدہ سپلائر قبولیت لفافے کو اس اسٹیجنگ ڈائرکٹری میں داخل کریں جس پر اشارہ کیا گیا ہے
   `torii.sorafs.discovery.admission.envelopes_dir`۔
3. ڈسکوری انٹرفیس کے ذریعہ فراہم کنندہ اشتہارات کے پھیلاؤ کو چیک کریں:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. گورننس ہیڈر کے ساتھ ٹیسٹ مینی فیسٹ/پلان پوائنٹس:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. اس بات کو یقینی بنائیں کہ ٹیلی میٹری پینل (`torii_sorafs_*`) اور الارم کے اڈے نئی فائل کو ظاہر کرتے ہیں
   غلطیوں کے بغیر

## 4. پروڈکشن لانچ

1. پروڈکشن نوڈ Torii پر اسٹیجنگ مراحل کو دہرائیں۔
2. آپریٹر چینلز اور ایس ڈی کے کے لئے ایکٹیویشن ونڈو (تاریخ/وقت ، فضل کی مدت ، رول بیک پلان) کا اعلان کریں۔
3. PR ورژن کو ضم کریں جس میں شامل ہیں:
   - تازہ ترین فکسچر اور حالات
   - دستاویزات میں تبدیلی (چارٹر حوالہ جات ، لازمی رپورٹ)
   - روڈ میپ/حیثیت کو اپ ڈیٹ کریں
4. ریلیز اور آرکائیو کو پروویژن مقاصد کے لئے دستخط شدہ ٹکڑوں کو ٹیگ کریں۔

## 5. لانچ کے بعد آڈٹ

1. فائنل میٹرکس پر قبضہ کریں (دریافت کاؤنٹرز ، کامیابی کی شرح ، ہسٹوگرام)
   غلطیاں) لانچ کے 24 گھنٹے بعد۔
2. ایک مختصر خلاصہ اور تعی .ن کی رپورٹ سے لنک کے ساتھ `status.md` کو اپ ڈیٹ کریں۔
3. `roadmap.md` میں کسی بھی فالو اپ ٹاسک (جیسے اضافی فائل لکھنے کی ہدایات) ریکارڈ کریں۔