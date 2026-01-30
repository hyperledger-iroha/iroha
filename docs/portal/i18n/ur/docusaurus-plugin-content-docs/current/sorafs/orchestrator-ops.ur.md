---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-ops.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: orchestrator-ops
title: SoraFS آرکسٹریٹر آپریشنز رن بک
sidebar_label: آرکسٹریٹر رن بک
description: ملٹی سورس آرکسٹریٹر کے رول آؤٹ، نگرانی اور رول بیک کے لیے مرحلہ وار آپریشنل گائیڈ۔
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md` کی عکاسی کرتا ہے۔ جب تک پرانا Sphinx ڈاکیومنٹیشن سیٹ مکمل طور پر منتقل نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

یہ رن بک SRE ٹیموں کو ملٹی سورس fetch آرکسٹریٹر کی تیاری، رول آؤٹ اور آپریشنز میں رہنمائی کرتی ہے۔ یہ ڈیولپر گائیڈ کو پروڈکشن رول آؤٹس کے مطابق طریقۂ کار سے مکمل کرتی ہے، جن میں مرحلہ وار فعال سازی اور peers کی بلیک لسٹنگ شامل ہے۔

> **مزید دیکھیں:** [ملٹی سورس رول آؤٹ رن بک](./multi-source-rollout.md) فلیٹ لیول رول آؤٹ ویوز اور ہنگامی حالات میں پرووائیڈرز کی روک تھام پر توجہ دیتی ہے۔ گورننس / اسٹیجنگ کی ہم آہنگی کے لیے اسے دیکھیں اور روزمرہ آرکسٹریٹر آپریشنز کے لیے اس دستاویز کا استعمال کریں۔

## 1. قبل از عمل چیک لسٹ

1. **پرووائیڈر ان پٹس جمع کریں**
   - ہدف فلیٹ کے لیے تازہ ترین پرووائیڈر اشتہارات (`ProviderAdvertV1`) اور ٹیلیمیٹری اسنیپ شاٹ۔
   - زیرِ آزمائش مینِفیسٹ سے اخذ کردہ payload پلان (`plan.json`)۔
2. **متعین (deterministic) اسکور بورڈ تیار کریں**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - تصدیق کریں کہ `artifacts/scoreboard.json` میں ہر پروڈکشن پرووائیڈر `eligible` کے طور پر درج ہے۔
   - اسکور بورڈ کے ساتھ سمری JSON آرکائیو کریں؛ آڈیٹرز تبدیلی کی درخواست کی توثیق میں chunk ریٹرائی کاؤنٹرز پر انحصار کرتے ہیں۔
3. **fixtures کے ساتھ dry-run** — `docs/examples/sorafs_ci_sample/` میں موجود عوامی fixtures پر یہی کمانڈ چلائیں تاکہ پروڈکشن payloads کو چھونے سے پہلے آرکسٹریٹر بائنری متوقع ورژن سے میچ ہو جائے۔

## 2. مرحلہ وار رول آؤٹ طریقۂ کار

1. **کینری مرحلہ (≤2 پرووائیڈرز)**
   - اسکور بورڈ دوبارہ بنائیں اور `--max-peers=2` کے ساتھ چلائیں تاکہ آرکسٹریٹر کو چھوٹے سب سیٹ تک محدود کیا جا سکے۔
   - مانیٹر کریں:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - اس وقت آگے بڑھیں جب مکمل مینِفیسٹ fetch کے لیے ریٹرائی ریٹس 1% سے کم رہیں اور کوئی پرووائیڈر ناکامیاں جمع نہ کرے۔
2. **رَیمپ مرحلہ (50% پرووائیڈرز)**
   - `--max-peers` بڑھائیں اور تازہ ٹیلیمیٹری اسنیپ شاٹ کے ساتھ دوبارہ چلائیں۔
   - ہر رن کو `--provider-metrics-out` اور `--chunk-receipts-out` کے ساتھ محفوظ کریں۔ آرٹیفیکٹس کو ≥7 دن محفوظ رکھیں۔
3. **مکمل رول آؤٹ**
   - `--max-peers` ہٹا دیں (یا اسے تمام اہل تعداد پر سیٹ کریں)۔
   - کلائنٹ ڈیپلائمنٹس میں آرکسٹریٹر موڈ فعال کریں: محفوظ شدہ اسکور بورڈ اور کنفیگریشن JSON کو اپنے کنفیگریشن مینجمنٹ سسٹم کے ذریعے تقسیم کریں۔
   - ڈیش بورڈز کو اپ ڈیٹ کریں تاکہ `sorafs_orchestrator_fetch_duration_ms` p95/p99 اور علاقے کے لحاظ سے ریٹرائی ہسٹوگرامز دکھائی دیں۔

## 3. peers کی بلیک لسٹنگ اور بوسٹنگ

CLI اسکورنگ پالیسی اوور رائیڈز استعمال کریں تاکہ گورننس اپ ڈیٹس کا انتظار کیے بغیر غیر صحت مند پرووائیڈرز کی درجہ بندی ہو سکے۔

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` درج شدہ عرف کو موجودہ سیشن میں غور سے ہٹا دیتا ہے۔
- `--boost-provider=<alias>=<weight>` پرووائیڈر کے شیڈیولر وزن میں اضافہ کرتا ہے۔ اقدار نارملائزڈ اسکور بورڈ وزن میں جمع ہوتی ہیں اور صرف لوکل رن پر لاگو ہوتی ہیں۔
- اوور رائیڈز کو انسیڈنٹ ٹکٹ میں درج کریں اور JSON آؤٹ پٹس منسلک کریں تاکہ بنیادی مسئلہ حل ہونے پر ذمہ دار ٹیم اسٹیٹ کو ہم آہنگ کر سکے۔

مستقل تبدیلیوں کے لیے سورس ٹیلیمیٹری میں ترمیم کریں (خلاف ورزی کرنے والے کو penalised کے طور پر نشان زد کریں) یا CLI اوور رائیڈز صاف کرنے سے پہلے اپ ڈیٹڈ اسٹریم بجٹس کے ساتھ اشتہار ریفریش کریں۔

## 4. خرابیوں کی جانچ

جب fetch ناکام ہو:

1. دوبارہ چلانے سے پہلے درج ذیل آرٹیفیکٹس اکٹھے کریں:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. `session.summary.json` میں قابل فہم ایرر سٹرنگ دیکھیں:
   - `no providers were supplied` → پرووائیڈر راستوں اور اشتہارات کی تصدیق کریں۔
   - `retry budget exhausted ...` → `--retry-budget` بڑھائیں یا غیر مستحکم peers ہٹا دیں۔
   - `no compatible providers available ...` → خلاف ورزی کرنے والے پرووائیڈر کی رینج کیپیبلٹی میٹاڈیٹا آڈٹ کریں۔
3. پرووائیڈر نام کو `sorafs_orchestrator_provider_failures_total` کے ساتھ ملائیں اور اگر میٹرک میں اچانک اضافہ ہو تو فالو اپ ٹکٹ بنائیں۔
4. `--scoreboard-json` اور حاصل شدہ ٹیلیمیٹری کے ساتھ آف لائن fetch چلا کر خرابی کو متعین طور پر دوبارہ پیدا کریں۔

## 5. رول بیک

آرکسٹریٹر رول آؤٹ واپس لینے کے لیے:

1. ایسی کنفیگریشن تقسیم کریں جو `--max-peers=1` سیٹ کرے (ملٹی سورس شیڈیولنگ کو مؤثر طور پر غیر فعال کرتی ہے) یا کلائنٹس کو پرانے سنگل سورس fetch راستے پر واپس لے جائیں۔
2. کسی بھی `--boost-provider` اوور رائیڈ کو ہٹا دیں تاکہ اسکور بورڈ نیوٹرل ویٹنگ پر واپس آ جائے۔
3. کم از کم ایک دن تک آرکسٹریٹر میٹرکس اکٹھے کرتے رہیں تاکہ یقین ہو جائے کہ کوئی باقی fetch کارروائیاں جاری نہیں ہیں۔

آرٹیفیکٹس کی منظم کیپچرنگ اور مرحلہ وار رول آؤٹس برقرار رکھنے سے یہ یقینی بنتا ہے کہ ملٹی سورس آرکسٹریٹر کو مختلف نوعیت کے پرووائیڈر فلیٹس میں محفوظ طریقے سے چلایا جا سکے، جبکہ آبزرویبیلٹی اور آڈٹ تقاضے برقرار رہیں۔
