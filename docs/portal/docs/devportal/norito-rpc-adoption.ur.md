---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
translator: Codex (automated)
source_hash: 39cbd5e448c8a868c50401c15466d7159cb08ad7be52dafb9e9dc66d5bba979d
source_last_modified: "2025-11-17T18:34:13.669847+00:00"
translation_last_reviewed: 2025-11-18
id: norito-rpc-adoption
title: Norito-RPC اپنانے کا شیڈول
sidebar_label: Norito-RPC اپنانا
description: کراس-SDK رول آؤٹ پلان، شواہد کی چیک لسٹ، اور NRPC-4 کے لیے آٹومیشن ہُکس۔
---

<div dir="rtl">

<!-- docs/portal/docs/devportal/norito-rpc-adoption.md کا اردو ترجمہ -->

# Norito-RPC اپنانے کا شیڈول

> بنیادی منصوبہ بندی کے نوٹس `docs/source/torii/norito_rpc_adoption_schedule.md`
> میں رہتے ہیں۔  
> یہ پورٹل کاپی SDK مصنفین، آپریٹرز، اور ریویوئرز کے لیے رول آؤٹ توقعات کو
> خلاصہ کرتی ہے۔

## مقاصد

- ہر SDK (Rust CLI، Python، JavaScript، Swift، Android) کو AND4 پروڈکشن ٹوگل سے
  پہلے بائنری Norito-RPC ٹرانسپورٹ پر ہم آہنگ کرنا۔
- فیز گیٹس، ثبوتی بنڈلز، اور ٹیلی میٹری ہُکس کو ڈیٹرمنسٹک رکھنا تاکہ گورننس رول
  آؤٹ کا آڈٹ کر سکے۔
- NRPC-4 میں درج مشترکہ ہیلپرز کے ساتھ فکسچر اور کینری ثبوت جمع کرنا آسان بنانا۔

## فیز ٹائم لائن

| مرحلہ | دورانیہ | دائرہ کار | اخراج کے معیار |
|-------|---------|-----------|-----------------|
| **P0 – لیب parity** | 2025 کی دوسری سہ ماہی | Rust CLI + Python smoke suites CI میں `/v1/norito-rpc` چلاتی ہیں، JS ہیلپر یونٹ ٹیسٹس پاس کرتا ہے، Android mock harness ڈوئل ٹرانسپورٹس exercise کرتا ہے۔ | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` اور `javascript/iroha_js/test/noritoRpcClient.test.js` CI میں سبز ہوں؛ Android harness `./gradlew test` میں وائر ہو۔ |
| **P1 – SDK preview** | 2025 کی تیسری سہ ماہی | مشترکہ فکسچر بنڈل چیک ان، `scripts/run_norito_rpc_fixtures.sh --sdk <label>` لاگز + JSON کو `artifacts/norito_rpc/` میں ریکارڈ کرتا ہے، SDK samples میں اختیاری Norito فلیگز نظروں پر ہوں۔ | فکسچر مینفسٹ سائن ہوا، README اپ ڈیٹس opt-in طریقہ دکھائیں، Swift preview API IOS2 فلیگ کے پیچھے دستیاب ہو۔ |
| **P2 – Staging / AND4 preview** | 2026 کی پہلی سہ ماہی | Staging Torii pools Norito کو ترجیح دیں، Android AND4 preview کلائنٹس اور Swift IOS2 parity suites ڈیفالٹاً بائنری ٹرانسپورٹ استعمال کریں، ٹیلی میٹری ڈیش بورڈ `dashboards/grafana/torii_norito_rpc_observability.json` بھرا ہوا ہو۔ | `docs/source/torii/norito_rpc_stage_reports.md` کینری کو دستاویزی کرے، `scripts/telemetry/test_torii_norito_rpc_alerts.sh` پاس کرے، Android mock harness replay کامیابی/غلطی دونوں کیسز ریکارڈ کرے۔ |
| **P3 – پروڈکشن GA** | 2026 کی چوتھی سہ ماہی | Norito تمام SDKs کے لیے ڈیفالٹ ٹرانسپورٹ بن جائے؛ JSON صرف فallback رہے۔ ریلیز جاب ہر ٹیگ کے ساتھ parity artefacts آرکائیو کریں۔ | ریلیز چیک لسٹ Rust/JS/Python/Swift/Android کے Norito smoke آؤٹ پٹ جمع کرے؛ Norito بمقابلہ JSON error-rate SLO کے لیے الرٹ تھریش ہولڈز نافذ ہوں؛ `status.md` اور ریلیز نوٹس GA ثبوت کا حوالہ دیں۔ |

## SDK ڈلیوریبلز اور CI ہُکس

- **Rust CLI اور integration harness** – `iroha_cli pipeline` smoke ٹیسٹس کو Norito ٹرانسپورٹ پر مجبور کریں جب `cargo xtask norito-rpc-verify` دستیاب ہو۔ لیب کے لیے `cargo test -p integration_tests -- norito_streaming` اور staging/GA کے لیے `cargo xtask norito-rpc-verify` چلائیں، artefacts کو `artifacts/norito_rpc/` میں محفوظ کریں۔
- **Python SDK** – ریلیز smoke (`python/iroha_python/scripts/release_smoke.sh`) کو Norito RPC پر ڈیفالٹ کریں، `run_norito_rpc_smoke.sh` کو CI entrypoint رکھیں، اور parity ہینڈلنگ کو `python/iroha_python/README.md` میں دستاویزی کریں۔ CI ہدف: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **JavaScript SDK** – `NoritoRpcClient` کو مستحکم کریں، جب `toriiClientConfig.transport.preferred === "norito_rpc"` ہو تو governance/query ہیلپرز Norito کو ڈیفالٹ کریں، اور `javascript/iroha_js/recipes/` میں end-to-end نمونے جمع کریں۔ CI کو `npm test` کے علاوہ docker شدہ `npm run test:norito-rpc` جاب بھی چلانی چاہیے؛ provenance `javascript/iroha_js/artifacts/` کے تحت Norito smoke لاگز اپ لوڈ کرے۔
- **Swift SDK** – IOS2 فلیگ کے پیچھے Norito bridge ٹرانسپورٹ وائر کریں، فکسچر cadence کو آئینہ کریں، اور Connect/Norito parity suite کو Buildkite lanes (`docs/source/sdk/swift/index.md`) میں چلائیں۔
- **Android SDK** – AND4 preview کلائنٹس اور mock Torii harness Norito اپنائیں، ریٹری/بیک آفس ٹیلی میٹری کو `docs/source/sdk/android/networking.md` میں دستاویزی کریں۔ harness مشترکہ فکسچرز کو `scripts/run_norito_rpc_fixtures.sh --sdk android` کے ذریعے استعمال کرے۔

## شواہد اور آٹومیشن

- `scripts/run_norito_rpc_fixtures.sh`، `cargo xtask norito-rpc-verify` کو ریپ کرتا ہے، stdout/stderr قید کرتا ہے، اور `fixtures.<sdk>.summary.json` آؤٹ پٹ دیتا ہے تاکہ SDK مالکان کے پاس `status.md` کے لیے ڈیٹرمنسٹک artefact ہو۔ `--sdk <label>` اور `--out artifacts/norito_rpc/<stamp>/` استعمال کریں۔
- `cargo xtask norito-rpc-verify` اسکیمہ ہیش parity (`fixtures/norito_rpc/schema_hashes.json`) نافذ کرتا ہے اور Torii کے `X-Iroha-Error-Code: schema_mismatch` لوٹانے پر ناکام ہوتا ہے۔ ہر ناکامی کے ساتھ JSON fallback کیپچر جوڑیں۔
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` اور `dashboards/grafana/torii_norito_rpc_observability.json`، NRPC-2 کے الرٹ کنٹریکٹس کی تعریف کرتے ہیں۔ ہر ڈیش بورڈ ترمیم کے بعد اسکرپٹ چلائیں اور `promtool` آؤٹ پٹ کو کینری بنڈل میں محفوظ کریں۔
- `docs/source/runbooks/torii_norito_rpc_canary.md` staging اور پروڈکشن ڈرلز بیان کرتا ہے؛ جب بھی فکسچر ہیش یا الرٹ گیٹس بدلیں اسے اپ ڈیٹ کریں۔

## ریویوئر چیک لسٹ

NRPC-4 مائل اسٹون پر دستخط کرنے سے پہلے تصدیق کریں:

1. تازہ ترین فکسچر بنڈل ہیشز `fixtures/norito_rpc/schema_hashes.json` اور متعلقہ CI artefact (`artifacts/norito_rpc/<stamp>/`) سے میل کھاتے ہوں۔
2. SDK README / پورٹل ڈاکس وضاحت کریں کہ JSON fallback کیسے فورس کرنا ہے اور Norito ڈیفالٹ کو حوالہ دیں۔
3. ٹیلی میٹری ڈیش بورڈز ڈوئل اسٹیک error-rate پینلز اور الرٹ لنکس دکھائیں، اور Alertmanager ڈرائی رن (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) ٹریکر کے ساتھ منسلک ہو۔
4. یہاں کا اپنانے کا شیڈول `docs/source/torii/norito_rpc_tracker.md` اور روڈ میپ (NRPC-4) کے ساتھ ہم آہنگ ہو اور وہی evidence bundle حوالہ دے۔

شیڈول پر قائم رہنے سے کراس-SDK رویہ قابلِ پیشن گوئی رہتا ہے اور گورننس کو Norito-RPC اپنانے کا آڈٹ بغیر اضافی درخواستوں کے انجام دینے میں مدد ملتی ہے۔

</div>
