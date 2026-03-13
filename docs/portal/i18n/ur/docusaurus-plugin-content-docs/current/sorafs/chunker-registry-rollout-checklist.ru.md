---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: چنکر رجسٹری-رول آؤٹ-چیک لسٹ
عنوان: چیک لسٹ رول آؤٹ رجسٹری چنکر SoraFS
سائڈبار_لیبل: رول آؤٹ چنکر چیک لسٹ
تفصیل: چنکر رجسٹری کی تازہ کاریوں کے لئے مرحلہ وار رول آؤٹ پلان۔
---

::: نوٹ کینونیکل ماخذ
`docs/source/sorafs/chunker_registry_rollout_checklist.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں ہم آہنگی کو برقرار رکھیں جب تک کہ اسفنکس دستاویزات کا سیٹ ریٹائر نہ ہو۔
:::

# چیک لسٹ رول آؤٹ رجسٹری SoraFS

اس چیک لسٹ نے ایک نئے چنکر پروفائل کو فروغ دینے کے لئے درکار اقدامات کو اپنی گرفت میں لیا ہے
یا توثیق کے بعد جائزہ سے پیداوار میں بنڈل فراہم کرنے والے کا داخلہ
گورننس چارٹر۔

> ** دائرہ کار: ** اس تبدیلی پر لاگو ہوتا ہے
> `sorafs_manifest::chunker_registry` ، فراہم کنندہ داخلہ لفافے یا
> کیننیکل فکسچر بنڈل (`fixtures/sorafs_chunker/*`)۔

## 1. پری توثیق

1. فکسچر کو دوبارہ تخلیق کریں اور عزم کی جانچ کریں:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. اس بات کو یقینی بنائیں کہ عزم ازم ہیش ہے
   `docs/source/sorafs/reports/sf1_determinism.md` (یا متعلقہ رپورٹ
   پروفائل) دوبارہ تخلیق شدہ نمونے کے ساتھ موافق ہے۔
3. یقینی بنائیں `sorafs_manifest::chunker_registry` کے ساتھ مرتب کریں
   `ensure_charter_compliance()` اسٹارٹ اپ پر:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. اپ ڈیٹ ڈوسیئر آفرز:
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` پر منٹ کے مشورے لکھیں
   - عزم پرستی سے متعلق رپورٹ

## 2. گورننس سائن آف

1. ٹولنگ ورکنگ گروپ رپورٹ اور پروپوزل ڈائجسٹ کو جمع کروائیں
   سورہ پارلیمنٹ انفراسٹرکچر پینل۔
2. ریکارڈ کی منظوری کی تفصیلات میں
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`۔
3. پارلیمنٹ کے ذریعہ دستخط شدہ لفافے کو فکسچر کے ساتھ شائع کریں:
   `fixtures/sorafs_chunker/manifest_signatures.json`۔
4. چیک کریں کہ گیٹ گورننس ہیلپر کے ذریعے لفافہ قابل رسائی ہے:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. اسٹیجنگ رول آؤٹ

تفصیلی واک تھرو کے لئے ، [اسٹیجنگ مینی فیسٹ پلے بک] (./staging-manifest-playbook) دیکھیں۔

1. Torii کو دریافت `torii.sorafs` فعال اور انفورسمنٹ قابل بنایا
   داخلہ (`enforce_admission = true`)۔
2. اسٹیجنگ رجسٹری ڈائرکٹری میں منظور شدہ فراہم کنندہ داخلہ لفافے اپ لوڈ کریں ،
   `torii.sorafs.discovery.admission.envelopes_dir` میں مخصوص ہے۔
3. چیک کریں کہ فراہم کنندہ اشتہارات ڈسکوری API کے ذریعے تقسیم کیے جاتے ہیں:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. گورننس ہیڈرز کے ساتھ اختتامی مقامات مینی فیسٹ/پلان:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. یقینی بنائیں کہ ٹیلی میٹری ڈیش بورڈز (`torii_sorafs_*`) اور الرٹ کے قواعد
   غلطیوں کے بغیر نیا پروفائل ڈسپلے کریں۔

## 4. پروڈکشن رول آؤٹ

1. پیداوار Torii نوڈس پر اسٹیجنگ مراحل کو دہرائیں۔
2. چینلز میں ایکٹیویشن ونڈو (تاریخ/وقت ، فضل کی مدت ، رول بیک پلان) کا اعلان کریں
   آپریٹرز اور ایس ڈی کے۔
3. انضمام کی رہائی PR کے ساتھ:
   - تازہ ترین فکسچر اور لفافہ
   - دستاویزی تبدیلیاں (چارٹر کے لنکس ، تعی .ن سے متعلق رپورٹ)
   - تازہ کاری شدہ روڈ میپ/حیثیت
4. ریلیز ٹیگ اور آرکائیو پر دستخط شدہ نمونے کو پیش کرنے کے لئے مرتب کریں۔

## 5. پوسٹ رول آؤٹ آڈٹ

1. حتمی میٹرکس لیں (دریافت کی گنتی ، کامیابی کی شرح ، غلطی
   ہسٹگرامس) رول آؤٹ کے 24 گھنٹے بعد۔
2. ایک مختصر خلاصہ اور تعی .ن کی رپورٹ سے لنک کے ساتھ `status.md` کو اپ ڈیٹ کریں۔
3. فالو اپ ٹاسک بنائیں (مثال کے طور پر ، تصنیف سے متعلق اضافی رہنمائی
   `roadmap.md` میں پروفائلز)۔