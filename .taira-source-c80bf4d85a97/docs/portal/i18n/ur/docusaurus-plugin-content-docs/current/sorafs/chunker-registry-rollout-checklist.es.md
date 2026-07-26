---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: چنکر رجسٹری-رول آؤٹ-چیک لسٹ
عنوان: SoraFS چنکر لاگ رول آؤٹ چیک لسٹ
سائڈبار_لیبل: چنکر رول آؤٹ چیک لسٹ
تفصیل: چنکر رجسٹری کی تازہ کاریوں کے لئے مرحلہ وار رول آؤٹ پلان۔
---

::: نوٹ کینونیکل ماخذ
`docs/source/sorafs/chunker_registry_rollout_checklist.md` کی عکاسی کرتا ہے۔ جب تک میراثی اسفنکس دستاویزات کا سیٹ ریٹائر نہیں ہوتا ہے تب تک دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

# SoraFS رجسٹری رول آؤٹ چیک لسٹ

اس چیک لسٹ نے ایک نئے چنکر پروفائل کو فروغ دینے کے لئے ضروری اقدامات کو اپنی گرفت میں لیا ہے
یا ایک سپلائر انٹیک بنڈل جائزہ سے لے کر پروڈکشن تک کے بعد
گورننس چارٹر کی توثیق کی گئی ہے۔

> ** دائرہ کار: ** ان تمام ریلیز پر لاگو ہوتا ہے جو ترمیم کرتے ہیں
> `sorafs_manifest::chunker_registry` ، سپلائر داخلہ لفافے یا
> کیننیکل فکسچر بنڈل (`fixtures/sorafs_chunker/*`)۔

## 1. پہلے توثیق

1. فکسچر کو دوبارہ تخلیق کریں اور عزم کی تصدیق کریں:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. تصدیق کریں کہ عزم پسندی ہیش میں ہے
   `docs/source/sorafs/reports/sf1_determinism.md` (یا متعلقہ پروفائل رپورٹ)
   تخلیق نو نمونے سے ملیں۔
3. یقینی بنائیں `sorafs_manifest::chunker_registry` کے ساتھ مرتب کریں
   `ensure_charter_compliance()` چل رہا ہے:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. پروپوزل ڈوزیئر کو اپ ڈیٹ کریں:
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` میں کونسل منٹ میں اندراج
   - تعی .ن کی رپورٹ

## 2۔ گورننس کی منظوری

1. ٹولنگ ورکنگ گروپ کی رپورٹ پیش کریں اور اس تجویز کو ہضم کریں
   سورہ پارلیمنٹ انفراسٹرکچر پینل۔
2. ریکارڈ کی منظوری کی تفصیلات میں
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`۔
3. پارلیمنٹ کے ذریعہ فکسچر کے ساتھ دستخط شدہ لفافے کو شائع کریں:
   `fixtures/sorafs_chunker/manifest_signatures.json`۔
4. تصدیق کریں کہ لفافہ گورننس کے ذریعہ قابل رسائی ہے۔
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. اسٹیجنگ میں رول آؤٹ

ایک کے لئے [منشور اسٹیجنگ پلے بوک] (./staging-manifest-playbook) دیکھیں
ان اقدامات کا تفصیلی واک تھرو۔

1. Torii کو دریافت `torii.sorafs` کے ساتھ فعال کریں اور قابل بنائیں
   داخلہ چالو (`enforce_admission = true`)۔
2. منظوری فراہم کرنے والوں سے رجسٹریشن ڈائرکٹری میں داخلہ لفافے اپ لوڈ کریں
   `torii.sorafs.discovery.admission.envelopes_dir` کے ذریعہ حوالہ دیا گیا ہے۔
3. تصدیق کریں کہ فراہم کنندہ اشتہارات کو دریافت API کے ذریعے پھیلایا جاتا ہے:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. گورننس ہیڈر کے ساتھ مینی فیسٹ/پلان اینڈ پوائنٹس چلائیں:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. تصدیق کریں کہ ٹیلی میٹری ڈیش بورڈز (`torii_sorafs_*`) اور کنٹرول کے قواعد
   انتباہ غلطیوں کے بغیر نئے پروفائل کی اطلاع دیں۔

## 4. پیداوار میں رول آؤٹ

1. پیداوار Torii نوڈس کے خلاف اسٹیجنگ اقدامات کو دہرائیں۔
2. ایکٹیویشن ونڈو کا اعلان (تاریخ/وقت ، فضل کی مدت ، رول بیک پلان)
   کیریئر چینلز اور ایس ڈی کے کو۔
3. ریلیز PR کو ضم کریں جس میں موجود ہے:
   - تازہ ترین فکسچر اور لفافے
   - دستاویزات میں تبدیلی (خط کا حوالہ ، تعی .ن کی رپورٹ)
   - روڈ میپ/اسٹیٹس ریفریش
4. ریلیز کو ٹیگ کریں اور پروویژن کے لئے دستخط شدہ نمونے کو محفوظ کریں۔

## 5. پوسٹ رول آؤٹ آڈٹ1. فائنل میٹرکس پر قبضہ کریں (دریافت کی گنتی ، کامیابی کی شرح بازیافت ،
   غلطی ہسٹگرامس) رول آؤٹ کے بعد 24 گھنٹہ۔
2. ایک مختصر خلاصہ اور تعی .ن کی رپورٹ کے لنک کے ساتھ `status.md` کو اپ ڈیٹ کریں۔
3. کسی بھی فالو اپ کاموں کو ریکارڈ کریں (جیسے مزید پروفائل تصنیف کی رہنمائی)
   `roadmap.md` پر۔