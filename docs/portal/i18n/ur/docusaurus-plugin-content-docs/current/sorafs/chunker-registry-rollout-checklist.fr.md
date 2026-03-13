---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: چنکر رجسٹری-رول آؤٹ-چیک لسٹ
عنوان: چنکر رجسٹری رول آؤٹ چیک لسٹ SoraFS
سائڈبار_لیبل: چیک لسٹ رول آؤٹ چنکر
تفصیل: چنکر رجسٹری کی تازہ کاریوں کے لئے مرحلہ وار رول آؤٹ پلان۔
---

::: نوٹ کینونیکل ماخذ
`docs/source/sorafs/chunker_registry_rollout_checklist.md` کی عکاسی کرتا ہے۔ جب تک میراثی اسفینکس سیٹ مکمل طور پر ریٹائر نہیں ہوتا ہے ، دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

# رجسٹری رول آؤٹ چیک لسٹ SoraFS

اس چیک لسٹ میں ایک نئے پروفائل کو فروغ دینے کے لئے ضروری اقدامات کی تفصیلات ہیں
چنکر یا فراہم کنندہ داخلہ کا بنڈل جرنل سے پروڈکشن کے بعد
گورننس چارٹر کی توثیق۔

> ** دائرہ کار: ** ان تمام ریلیز پر لاگو ہوتا ہے جو ترمیم کرتے ہیں
> `sorafs_manifest::chunker_registry` ، سپلائر داخلہ لفافے یا
> کیننیکل فکسچر بنڈل (`fixtures/sorafs_chunker/*`)۔

## 1. ابتدائی توثیق

1. فکسچر کو دوبارہ تخلیق کریں اور عزم کی جانچ کریں:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. تصدیق کریں کہ عزم پسندی ہیش میں ہے
   `docs/source/sorafs/reports/sf1_determinism.md` (یا پروفائل رپورٹ
   متعلقہ) دوبارہ پیدا ہونے والے نمونے کے مطابق۔
3. یقینی بنائیں `sorafs_manifest::chunker_registry` کے ساتھ مرتب کریں
   `ensure_charter_compliance()` لانچ کرکے:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. پروپوزل فائل کو اپ ڈیٹ کریں:
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` کے تحت بورڈ منٹ میں داخلہ
   - تعی .ن کی رپورٹ

## 2۔ گورننس کی توثیق

1. ٹولنگ ورکنگ گروپ رپورٹ اور پروپوزل ڈائجسٹ پیش کریں
   سورہ پارلیمنٹ انفراسٹرکچر پینل میں۔
2. منظوری کی تفصیلات کو محفوظ کریں
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`۔
3. فکسچر کے ساتھ ہی پارلیمنٹ کے ذریعہ دستخط شدہ لفافے کو شائع کریں:
   `fixtures/sorafs_chunker/manifest_signatures.json`۔
4. تصدیق کریں کہ لفافہ گورننس کے ذریعہ قابل رسائی ہے۔
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. رول آؤٹ اسٹیجنگ

ایک کے لئے [پلے بوک مینی فیسٹ اسٹیجنگ] (./staging-manifest-playbook) کا حوالہ دیں
تفصیلی طریقہ کار۔

1. Torii کو دریافت `torii.sorafs` فعال اور درخواست کے ساتھ تعینات کریں
   داخلہ چالو (`enforce_admission = true`)۔
2. سپلائی کے حامل لفافوں کو ڈائریکٹری میں دبائیں
   رجسٹری `torii.sorafs.discovery.admission.envelopes_dir` کے ذریعہ حوالہ دیا گیا ہے۔
3. تصدیق کریں کہ فراہم کنندہ کے اشتہارات کو دریافت API کے ذریعے پھیلایا جاتا ہے:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. گورننس ہیڈر کے ساتھ ورزش کے ظاہر/منصوبہ بندی کے اختتامی مقامات:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. تصدیق کریں کہ ٹیلی میٹری ڈیش بورڈز (`torii_sorafs_*`) اور قواعد
   انتباہ غلطیوں کے بغیر نئے پروفائل کی اطلاع دیتا ہے۔

## 4. رول آؤٹ پروڈکشن

1. پیداوار Torii نوڈس پر اسٹیجنگ مراحل کو دہرائیں۔
2. ایکٹیویشن ونڈو (تاریخ/وقت ، فضل کی مدت ، رول بیک پلان) کا اعلان کریں
   آپریٹر چینلز اور ایس ڈی کے کو۔
3. ریلیز PR کو ضم کریں جس میں:
   - تازہ ترین فکسچر اور لفافہ
   - دستاویزات میں تبدیلی (چارٹر کے حوالے ، تعی .ن کی رپورٹ)
   - روڈ میپ/حیثیت کو تازہ کریں
4. ریلیز کو ٹیگ کریں اور پروویژن کے لئے دستخط شدہ نمونے میں چیک کریں۔

## 5. پوسٹ رول آؤٹ آڈٹ1. فائنل میٹرکس پر قبضہ کریں (دریافت کی گنتی ، کامیابی کی شرح بازیافت ،
   غلطی ہسٹگرامس) رول آؤٹ کے 24 گھنٹے بعد۔
2. ایک مختصر خلاصہ اور تعی .ن کی رپورٹ سے لنک کے ساتھ `status.md` کو اپ ڈیٹ کریں۔
3. ریکارڈ فالو اپ ٹاسک (جیسے پروفائل تصنیف کی رہنمائی)
   `roadmap.md`۔