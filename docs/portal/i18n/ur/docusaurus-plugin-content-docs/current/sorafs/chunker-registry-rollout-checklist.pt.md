---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: چنکر رجسٹری-رول آؤٹ-چیک لسٹ
عنوان: SoraFS چنکر رجسٹری رول آؤٹ چیک لسٹ
سائڈبار_لیبل: چنکر رول آؤٹ چیک لسٹ
تفصیل: چنکر رجسٹری کی تازہ کاریوں کے لئے مرحلہ وار رول آؤٹ پلان۔
---

::: نوٹ کینونیکل ماخذ
`docs/source/sorafs/chunker_registry_rollout_checklist.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

# SoraFS رجسٹری رول آؤٹ چیک لسٹ

اس چیک لسٹ نے ایک نئے چنکر پروفائل کو فروغ دینے کے لئے ضروری اقدامات کو اپنی گرفت میں لیا ہے
یا چارٹر کے بعد پیداوار کے لئے فراہم کنندہ کے داخلے کے بنڈل کا جائزہ لیں
گورننس کی توثیق کی گئی ہے۔

> ** دائرہ کار: ** ان تمام ریلیز پر لاگو ہوتا ہے جو ترمیم کرتے ہیں
> `sorafs_manifest::chunker_registry` ، فراہم کنندہ داخلہ لفافے ، یا بنڈل
> کیننیکل فکسچر (`fixtures/sorafs_chunker/*`) سے۔

## 1. پرواز سے پہلے کی توثیق

1. فکسچر کو دوبارہ تخلیق کریں اور عزم کی جانچ کریں:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. تصدیق کریں کہ عزم پسندی ہیش میں ہے
   `docs/source/sorafs/reports/sf1_determinism.md` (یا پروفائل رپورٹ
   متعلقہ) دوبارہ تخلیق شدہ نمونے سے ملیں۔
3. اس بات کو یقینی بنائیں کہ `sorafs_manifest::chunker_registry` کے ساتھ مرتب کریں
   `ensure_charter_compliance()` چل رہا ہے:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. پروپوزل ڈوزیئر کو اپ ڈیٹ کریں:
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` میں بورڈ منٹ میں داخلہ
   - تعی .ن کی رپورٹ

## 2. گورننس سائن آف

1. ٹولنگ ورکنگ گروپ رپورٹ اور پروپوزل ڈائجسٹ کو پیش کریں
   سورہ پارلیمنٹ انفراسٹرکچر پینل۔
2. ریکارڈ کی منظوری کی تفصیلات میں
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`۔
3. پارلیمنٹ کے ذریعہ فکسچر کے ساتھ دستخط شدہ لفافے کو شائع کریں:
   `fixtures/sorafs_chunker/manifest_signatures.json`۔
4. چیک کریں کہ آیا لفافہ گورننس بازیافت مددگار کے ذریعہ قابل رسائی ہے یا نہیں:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. اسٹیجنگ رول آؤٹ

ایک کے لئے [اسٹیجنگ مینی فیسٹ پلے بک] (./staging-manifest-playbook) دیکھیں
تفصیلی قدم بہ قدم۔

1. Torii `torii.sorafs` دریافت کے ساتھ امپلانٹ فعال اور داخلہ نافذ کرنے والے
   منسلک (`enforce_admission = true`)۔
2. رجسٹری ڈائرکٹری میں منظور شدہ فراہم کنندہ داخلہ لفافے بھیجیں
   `torii.sorafs.discovery.admission.envelopes_dir` کے ذریعہ حوالہ دیا گیا ہے۔
3. چیک کریں کہ کون سا فراہم کنندہ ڈسکوری API کے ذریعے پروپیگنڈہ کرتا ہے:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. گورننس ہیڈر کے ساتھ ورزش کے ظاہر/منصوبہ بندی کے اختتامی مقامات:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. اس بات کی تصدیق کریں کہ ٹیلی میٹری ڈیش بورڈز (`torii_sorafs_*`) اور الرٹ کے قواعد
   غلطیوں کے بغیر نئے پروفائل کی اطلاع دیں۔

## 4. پیداوار میں رول آؤٹ

1. پیداوار Torii نوڈس پر اسٹیجنگ مراحل کو دہرائیں۔
2. میں ایکٹیویشن ونڈو (تاریخ/وقت ، فضل کی مدت ، رول بیک پلان) کا اعلان کریں
   آپریٹر چینلز اور ایس ڈی کے۔
3. ریلیز PR کو ضم کریں جس میں:
   - تازہ ترین فکسچر اور لفافہ
   - دستاویزات میں تبدیلیاں (چارٹر کے حوالے ، عزم کی رپورٹ)
   - روڈ میپ/اسٹیٹس ریفریش
4. ریلیز کو ٹیگ کریں اور پروویژن کے لئے دستخط شدہ نمونے کو محفوظ کریں۔

## 5. پوسٹ رول آؤٹ آڈٹ1. فائنل میٹرکس پر قبضہ کریں (دریافت گنتی ، کامیابی کی شرح ، ہسٹگرام)
   غلطی) رول آؤٹ کے بعد 24 ایچ۔
2. ایک مختصر خلاصہ اور تعی .ن کی رپورٹ سے لنک کے ساتھ `status.md` کو اپ ڈیٹ کریں۔
3. ریکارڈ فالو اپ ٹاسک (جیسے ، تصنیف کے لئے اضافی رہنمائی
   `roadmap.md` میں پروفائلز)