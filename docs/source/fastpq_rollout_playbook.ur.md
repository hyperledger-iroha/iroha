---
lang: ur
direction: rtl
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:26:20.579700+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# فاسٹ پی کیو رول آؤٹ پلے بک (اسٹیج 7-3)

یہ پلے بک اسٹیج 7-3 روڈ میپ کی ضرورت کو نافذ کرتی ہے: ہر بیڑے کو اپ گریڈ
جو فاسٹ پی کیو جی پی یو کو قابل بناتا ہے اس کو قابل تولیدی بینچ مارک منشور سے منسلک کرنا ہوگا ،
جوڑا Grafana ثبوت ، اور ایک دستاویزی رول بیک ڈرل۔ یہ تکمیل ہوتا ہے
`docs/source/fastpq_plan.md` (اہداف/فن تعمیر) اور
`docs/source/fastpq_migration_guide.md` (نوڈ سطح کے اپ گریڈ اقدامات) توجہ مرکوز کرکے
آپریٹر کا سامنا کرنے والی رول آؤٹ چیک لسٹ میں۔

## دائرہ کار اور کردار

- ** ریلیز انجینئرنگ / ایس آر ای: ** اپنے بینچ مارک کیپچرز ، ظاہر دستخط ، اور
  رول آؤٹ منظوری سے قبل ڈیش بورڈ برآمدات۔
- ** اوپس گلڈ: ** رنز اسٹیجڈ رول آؤٹ ، ریکارڈ رول بیک ریہرسلز ، اور اسٹورز
  `artifacts/fastpq_rollouts/<timestamp>/` کے تحت نوادرات کا بنڈل۔
- ** گورننس / تعمیل: ** تصدیق کرتا ہے کہ ثبوت ہر تبدیلی کے ساتھ ہیں
  فاسٹ پی کیو ڈیفالٹ سے پہلے درخواست کو بیڑے کے لئے ٹوگل کیا جاتا ہے۔

## ثبوت کے بنڈل کی ضروریات

ہر رول آؤٹ جمع کرانے میں مندرجہ ذیل نوادرات پر مشتمل ہونا چاہئے۔ تمام فائلوں کو منسلک کریں
ریلیز/اپ گریڈ ٹکٹ پر اور بنڈل کو اندر رکھیں
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`۔| نوادرات | مقصد | کیسے پیدا کریں |
| ---------- | --------- | ------------------ |
| `fastpq_bench_manifest.json` | یہ ثابت کرتا ہے کہ کیننیکل 20000-قطار کا کام کا بوجھ `<1 s` LDE چھت کے تحت رہتا ہے اور ہر لپیٹے ہوئے بینچ مارک کے لئے ہیشوں کو ریکارڈ کرتا ہے۔ دھات/CUDA رنز پر قبضہ کریں ، ان کو لپیٹیں ، پھر چلائیں:  `cargo xtask fastpq-bench-manifest \`  `  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \`  `  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \`  `  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \`  `  --signing-key secrets/fastpq_bench.ed25519 \`  Prometheus
| لپیٹے ہوئے بینچ مارک (`fastpq_metal_bench_*.json` ، `fastpq_cuda_bench_*.json`) | میزبان میٹا ڈیٹا ، قطار کے استعمال کے ثبوت ، صفر سے بھرنے والے ہاٹ سپاٹ ، پوسیڈن مائکروبینچ کے خلاصے ، اور ڈیش بورڈز/الرٹس کے ذریعہ استعمال ہونے والے دانا کے اعدادوشمار پر قبضہ کریں۔ `fastpq_metal_bench` / `fastpq_cuda_bench` چلائیں ، پھر را json کو لپیٹیں:  `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \`  `  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \`  `  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \`  `  fastpq_metal_bench.json artifacts/fastpq_benchmarks/<metal>.json --sign-output`  CUDA کی گرفتاری `--poseidon-metrics` متعلقہ گواہ/کھرچنے والی فائلوں میں)۔ مددگار فلٹرڈ `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` نمونے سرایت کرتا ہے لہذا WP2-E.6 ثبوت دھات اور CUDA میں ایک جیسے ہیں۔ جب آپ کو اسٹینڈ لون پوسیڈن مائکروبینچ سمری (لپیٹے ہوئے یا خام آدانوں کی تائید) کی ضرورت ہو تو `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` استعمال کریں۔ |
|  |  | ** اسٹیج 7 لیبل کی ضرورت: ** `wrap_benchmark.py` اب اس وقت تک ناکام ہوجاتا ہے جب تک کہ نتیجہ `metadata.labels` سیکشن میں `device_class` اور `gpu_kind` دونوں شامل نہ ہوں۔ جب خود کار طریقے سے پتہ لگانے سے ان کا اندازہ نہیں ہوسکتا (مثال کے طور پر ، جب کسی علیحدہ سی آئی نوڈ پر لپیٹتے ہو) تو ، واضح اوور رائڈس کو پاس کریں جیسے `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete`۔ |
|  |  | ** ایکسلریشن ٹیلی میٹری: ** ریپر `cargo xtask acceleration-state --format json` کو ڈیفالٹ کے ذریعہ بھی پکڑتا ہے ، `<bundle>.accel.json` اور `<bundle>.accel.prom` کو لپیٹے ہوئے بینچ مارک کے ساتھ لکھتا ہے (Prometheus کے ساتھ اوور رائڈ)۔ کیپچر میٹرکس ان فائلوں کو فلیٹ ڈیش بورڈز کے لئے `acceleration_matrix.{json,md}` بنانے کے لئے استعمال کرتا ہے۔ |
| Grafana برآمد | رول آؤٹ ونڈو کے لئے اپنانے والے ٹیلی میٹری اور الرٹ تشریحات کو ثابت کرتا ہے `fastpq-acceleration` ڈیش بورڈ برآمد کریں:  `curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \`  `  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \`  `  | jq '.dashboard' \`  `  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`  برآمد کرنے سے پہلے رول آؤٹ اسٹارٹ/اسٹاپ ٹائم کے ساتھ بورڈ کو بیان کریں۔ ریلیز پائپ لائن `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (`GRAFANA_TOKEN` کے ذریعے فراہم کردہ ٹوکن) کے ذریعے خود بخود یہ کام کر سکتی ہے۔ |
| الرٹ اسنیپ شاٹ | انتباہ کے قواعد پر قبضہ کرتا ہے جس نے رول آؤٹ کی حفاظت کی۔ | `dashboards/alerts/fastpq_acceleration_rules.yml` (اور `tests/` فکسچر) کو بنڈل میں کاپی کریں تاکہ جائزہ لینے والے `promtool test rules …` کو دوبارہ چلاسکیں۔ |
| رول بیک ڈرل لاگ | یہ ظاہر کرتا ہے کہ آپریٹرز نے جبری سی پی یو فال بیک اور ٹیلی میٹری کے اعترافات کی مشق کی [رول بیک مشق] (#rollback-drills) اور اسٹور کنسول لاگ (`rollback_drill.log`) کے علاوہ Prometheus سکریپ (`metrics_rollback.prom`) میں طریقہ کار استعمال کریں۔ || `row_usage/fastpq_row_usage_<date>.json` | ایگزیکٹوون فاسٹ پی کیو قطار مختص ریکارڈ کرتا ہے جو TF-5 سی آئی اور ڈیش بورڈز میں ٹریک کرتا ہے۔ Torii سے ایک تازہ گواہ ڈاؤن لوڈ کریں ، اسے `iroha_cli audit witness --decode exec.witness` کے ذریعے ڈیکوڈ کریں (متوقع پیرامیٹر سیٹ پر زور دینے کے لئے `--fastpq-parameter fastpq-lane-balanced` کو اختیاری طور پر شامل کریں ؛ فاسٹ پی کیو بیچز کو پہلے سے طے شدہ طور پر اخراج) ، اور `row_usage` JSON کو `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/row_usage/` میں کاپی کریں۔ فائل ناموں کو ٹائم اسٹیمپ رکھیں تاکہ جائزہ لینے والے ان کو رول آؤٹ ٹکٹ سے منسلک کرسکیں ، اور `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (یا `make check-fastpq-rollout`) چلائیں لہذا اسٹیج 7-3 گیٹ اس بات کی تصدیق کرتا ہے کہ ہر بیچ شواہد سے منسلک ہونے سے پہلے سلیکٹر کی گنتی اور `transfer_ratio = transfer_rows / total_rows` انوویرینٹ کی تشہیر کرتا ہے۔ |

> ** اشارہ: ** `artifacts/fastpq_rollouts/README.md` دستاویزات ترجیحی نام
> اسکیم (`<stamp>/<fleet>/<lane>`) اور مطلوبہ ثبوت فائلیں۔
> `<stamp>` فولڈر کو `YYYYMMDDThhmmZ` کو انکوڈ کرنا ضروری ہے تاکہ نوادرات ترتیب کے قابل رہیں
> ٹکٹوں سے مشورہ کیے بغیر۔

## ثبوت جنریشن چیک لسٹ1. ** جی پی یو بینچ مارک پر قبضہ کریں۔ **
   - کیننیکل ورک بوجھ (20000 منطقی قطاریں ، 32768 پیڈڈ قطاریں) چلائیں
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`۔
   - `scripts/fastpq/wrap_benchmark.py` کے ساتھ نتیجہ `--row-usage <decoded witness>` کا استعمال کرتے ہوئے لپیٹیں تاکہ بنڈل GPU ٹیلی میٹری کے ساتھ ساتھ گیجٹ شواہد بھی رکھتا ہو۔ `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` پاس کریں لہذا ریپر تیزی سے ناکام ہوجاتا ہے اگر یا تو ایکسلریٹر ہدف سے زیادہ ہو یا اگر پوسیڈن قطار/پروفائل ٹیلی میٹری غائب ہو ، اور علیحدہ دستخط تیار کرنے کے لئے۔
   - کوڈا کے میزبان پر دہرائیں تاکہ مینی فیسٹ میں دونوں جی پی یو خاندان ہوں۔
   - کریں ** نہیں ** `benchmarks.metal_dispatch_queue` کو چھین لیں
     `benchmarks.zero_fill_hotspots` لپیٹے JSON سے بلاکس۔ CI گیٹ
     (`ci/check_fastpq_rollout.sh`) اب ان فیلڈز کو پڑھتا ہے اور قطار میں ناکام ہوجاتا ہے
     ہیڈ روم ایک سلاٹ کے نیچے گرتا ہے یا جب کوئی ایل ڈی ای ہاٹ اسپاٹ رپورٹس `مطلب_مز>
     0.40ms` ، اسٹیج 7 ٹیلی میٹری گارڈ کو خود بخود نافذ کرنا۔
2. ** مینی فیسٹ پیدا کریں
   جدول میں دکھایا گیا ہے۔ `fastpq_bench_manifest.json` کو رول آؤٹ بنڈل میں اسٹور کریں۔
3. ** برآمد Grafana. **
   - `FASTPQ Acceleration Overview` بورڈ کو رول آؤٹ ونڈو کے ساتھ تشریح کریں ،
     متعلقہ Grafana پینل IDs سے لنک کرنا۔
   - Grafana API (اوپر کمانڈ) کے ذریعے ڈیش بورڈ JSON برآمد کریں اور شامل کریں
     `annotations` سیکشن تاکہ جائزہ لینے والے گود لینے کے منحنی خطوط سے مل سکیں
     اسٹیج رول آؤٹ۔
4. ** اسنیپ شاٹ الرٹس۔
   بنڈل میں رول آؤٹ کے ذریعہ۔ اگر Prometheus قواعد کو ختم کردیا گیا تو ، شامل کریں
   اوور رائڈ مختلف ہے۔
5. ** Prometheus/اوٹیل کھرچنی
   میزبان (اسٹیج سے پہلے اور بعد میں) نیز اوٹیل کاؤنٹر
   `fastpq.execution_mode_resolutions_total` اور جوڑا بنا ہوا
   `telemetry::fastpq.execution_mode` لاگ اندراجات۔ یہ نوادرات یہ ثابت کرتے ہیں
   جی پی یو کو اپنانا مستحکم ہے اور اس سے جب بھی سی پی یو فال بیکس کو ٹیلی میٹری کا اخراج ہوتا ہے۔
6. ** آرکائیو قطار استعمال کا ٹیلی میٹری۔
   رول آؤٹ ، نتیجے میں JSON کو بنڈل میں `row_usage/` کے تحت چھوڑیں۔ CI
   ہیلپر (`ci/check_fastpq_row_usage.sh`) ان سنیپ شاٹس کا موازنہ کرتا ہے
   کیننیکل بیس لائنز ، اور `ci/check_fastpq_rollout.sh` اب ہر ایک کی ضرورت ہے
   TF-5 شواہد کو منسلک رکھنے کے لئے کم از کم ایک `row_usage` فائل بھیجنے کے لئے بنڈل
   ریلیز ٹکٹ کے لئے.

## اسٹیج رول آؤٹ فلو

ہر بیڑے کے ل three تین عصبی مراحل کا استعمال کریں۔ باہر نکلنے کے بعد ہی آگے بڑھیں
ہر مرحلے میں معیارات مطمئن اور ثبوت کے بنڈل میں دستاویزی ہیں۔| مرحلہ | دائرہ کار | باہر نکلنے کے معیارات | منسلکات |
| ------- | ------- | --------------- | ------------- |
| پائلٹ (P1) | 1 کنٹرول ہوائی جہاز + 1 ڈیٹا ہوائی جہاز نوڈ فی خطہ | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` 48H ، صفر الرٹ مینجر واقعات ، اور پاسنگ رول بیک ڈرل کے لئے 90 ٪۔ | دونوں میزبانوں کا بنڈل (بینچ جےسن ، Grafana پائلٹ تشریح کے ساتھ ایکسپورٹ ، رول بیک لاگ)۔ |
| ریمپ (P2) | vide50 ٪ جائزوں کے علاوہ کم از کم ایک آرکائیول لین فی کلسٹر | جی پی یو کی پھانسی 5 دن تک برقرار رہی ، 1 سے زیادہ ڈاون گریڈ اسپائک> 10 منٹ ، اور Prometheus کاؤنٹرز 60 کی دہائی کے اندر فال بیکس کو الرٹ ثابت کرتے ہیں۔ | تازہ ترین Grafana ریمپ تشریح ، Prometheus سکریپ ڈفنس ، الرٹ مینجر اسکرین شاٹ/لاگ کو دکھا رہا ہے۔ |
| ڈیفالٹ (P3) | باقی نوڈس ؛ فاسٹ پی کیو `iroha_config` میں ڈیفالٹ کو نشان زد کیا گیا دستخط شدہ بینچ مینی فیسٹ + Grafana برآمدی حتمی گود لینے کے منحنی خطوط کا حوالہ دیتے ہوئے ، اور دستاویزی رول بیک ڈرل جس میں کنفیگ ٹوگل کا مظاہرہ کیا گیا ہے۔ | حتمی مینی فیسٹ ، Grafana JSON ، رول بیک لاگ ، CONFIG CHAIG CHAIG CHARE COMET COMENT کا حوالہ۔ |

رول آؤٹ ٹکٹ میں ہر فروغ کے ہر مرحلے کی دستاویز کریں اور براہ راست اس سے لنک کریں
`grafana_fastpq_acceleration.json` تشریحات تاکہ جائزہ لینے والے اس سے باہمی تعلق کرسکیں
ثبوت کے ساتھ ٹائم لائن.

## رول بیک مشقیں

ہر رول آؤٹ مرحلے میں ایک رول بیک ریہرسل شامل ہونا ضروری ہے:

1. فی کلسٹر ایک نوڈ منتخب کریں اور موجودہ میٹرکس کو ریکارڈ کریں:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2۔ کنفیگ نوب کا استعمال کرتے ہوئے 10 منٹ کے لئے سی پی یو وضع کو مجبور کریں
   (`zk.fastpq.execution_mode = "cpu"`) یا ماحولیات کو ختم کرنا:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. ڈاون گریڈ لاگ کی تصدیق کریں
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) اور کھرچنی
   کاؤنٹر انکریمنٹ کو ظاہر کرنے کے لئے ایک بار پھر Prometheus اختتامی نقطہ۔
4. جی پی یو وضع کو بحال کریں ، تصدیق کریں کہ `telemetry::fastpq.execution_mode` اب رپورٹ کرتا ہے
   `resolved="metal"` (یا غیر دھات لینوں کے لئے `resolved="cuda"/"opencl"`) ،
   تصدیق کریں Prometheus کھرچنی میں CPU اور GPU دونوں نمونے شامل ہیں
   `fastpq_execution_mode_total{backend=…}` ، اور گزرے ہوئے وقت کو لاگ ان کریں
   پتہ لگانے/صفائی۔
5. اسٹور شیل ٹرانسکرپٹس ، میٹرکس ، اور آپریٹر کے اعترافات جیسے
   `rollback_drill.log` اور `metrics_rollback.prom` رول آؤٹ بنڈل میں۔ یہ
   فائلوں کو مکمل ڈاون گریڈ + بحالی سائیکل کو واضح کرنا ہوگا کیونکہ
   `ci/check_fastpq_rollout.sh` جب بھی لاگ ان جی پی یو کی کمی ہوتا ہے تو اب ناکام ہوجاتا ہے
   بازیابی لائن یا میٹرکس اسنیپ شاٹ یا تو سی پی یو یا جی پی یو کاؤنٹرز کو چھوڑ دیتا ہے۔

ان نوشتوں سے یہ ثابت ہوتا ہے کہ ہر کلسٹر خوبصورتی سے ہراساں ہوسکتا ہے اور وہ ایس آر ای ٹیمیں
اگر جی پی یو ڈرائیور یا داناوں کو رجعت پسند ہے تو عزم کے ساتھ پیچھے گرنے کا طریقہ جانیں۔

## مخلوط-موڈ فال بیک بیک ثبوت (WP2-E.6)

جب بھی کسی میزبان کو GPU FFT/LDE کی ضرورت ہوتی ہے لیکن CPU پوسیڈن ہیشنگ (اسٹیج 7 <900ms کے مطابق)
ضرورت) ، معیاری رول بیک لاگوں کے ساتھ ساتھ مندرجہ ذیل نوادرات کو بنڈل کریں:1. ** کنفیگ ڈف.
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) رخصت ہوتے ہوئے
   `zk.fastpq.execution_mode` اچھوت۔ پیچ کا نام دیں
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`۔
2. ** پوسیڈن کاؤنٹر کھرچنا۔ **
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   گرفتاری کو لاک مرحلے میں `path="cpu_forced"` میں اضافہ کرنا چاہئے
   اس ڈیوائس کلاس کے لئے GPU FFT/LDE کاؤنٹر۔ پلٹ جانے کے بعد دوسرا کھرچنا لیں
   GPU وضع پر واپس جائیں تاکہ جائزہ لینے والے `path="gpu"` قطار دوبارہ شروع دیکھیں۔

   نتیجے میں فائل کو `wrap_benchmark.py --poseidon-metrics …` پر پاس کریں لہذا لپیٹے ہوئے بینچ مارک اپنے `poseidon_metrics` سیکشن کے اندر ایک ہی کاؤنٹرز کو ریکارڈ کرتا ہے۔ یہ ایک جیسے ورک فلو پر دھات اور CUDA رول آؤٹ رکھتا ہے اور الگ الگ سکریپ فائلوں کو کھولے بغیر فال بیک شواہد کو قابل اظہار بنا دیتا ہے۔
3. ** لاگ انرپٹ۔ ** `telemetry::fastpq.poseidon` اندراجات کو کاپی کریں
   ریزولور سی پی یو (`cpu_forced`) میں پلٹ گیا
   `poseidon_fallback.log` ، ٹائم اسٹیمپ کو رکھنا تاکہ الرٹ مینجر ٹائم لائنز ہوسکتی ہیں
   کنفیگ چینج کے ساتھ وابستہ ہے۔

CI آج قطار/صفر سے بھرنے والے چیکوں کو نافذ کرتا ہے۔ ایک بار مخلوط موڈ گیٹ اترتا ہے ،
`ci/check_fastpq_rollout.sh` بھی اصرار کرے گا کہ کسی بھی بنڈل پر مشتمل ہے
`poseidon_fallback.patch` مماثل `metrics_poseidon.prom` اسنیپ شاٹ کو بھیجتا ہے۔
اس ورک فلو کے بعد WP2-E.6 فال بیک بیک پالیسی قابل آڈٹ اور اس سے منسلک ہے
پہلے سے طے شدہ رول آؤٹ کے دوران استعمال ہونے والے وہی ثبوت جمع کرنے والے۔

## رپورٹنگ اور آٹومیشن

- پوری `artifacts/fastpq_rollouts/<stamp>/` ڈائرکٹری کو منسلک کریں
  ایک بار رول آؤٹ بند ہونے کے بعد ٹکٹ جاری کریں اور اس کا حوالہ `status.md` سے کریں۔
- `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` چلائیں (ویا
  `promtool`) CI کے اندر انتباہ بنڈل کو یقینی بنانے کے لئے رول آؤٹ کے ساتھ بنڈل بنڈل
  مرتب کریں۔
- `ci/check_fastpq_rollout.sh` (OR کے ساتھ بنڈل کی توثیق کریں
  `make check-fastpq-rollout`) اور جب آپ `FASTPQ_ROLLOUT_BUNDLE=<path>` کو پاس کریں
  ایک ہی رول آؤٹ کو نشانہ بنانا چاہتے ہیں۔ سی آئی نے اسی اسکرپٹ کے ذریعے درخواست کی ہے
  `.github/workflows/fastpq-rollout.yml` ، لہذا گمشدہ نوادرات a سے پہلے تیزی سے ناکام ہوجاتے ہیں
  ریلیز ٹکٹ بند ہوسکتا ہے۔ ریلیز پائپ لائن توثیق شدہ بنڈل آرکائو کرسکتی ہے
  گزر کر دستخط شدہ ظاہر کے ساتھ ساتھ
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` to
  `scripts/run_release_pipeline.py` ؛ مددگار دوبارہ کام کرتا ہے
  `ci/check_fastpq_rollout.sh` (جب تک کہ `--skip-fastpq-rollout-check` سیٹ نہیں ہوتا ہے) اور
  ڈائریکٹری کے درخت کو `artifacts/releases/<version>/fastpq_rollouts/…` میں کاپی کرتا ہے۔
  اس گیٹ کے ایک حصے کے طور پر اسکرپٹ اسٹیج 7 کی قطار کی گہرائی اور صفر سے بھر پور ہے
  `benchmarks.metal_dispatch_queue` اور پڑھ کر بجٹ
  `benchmarks.zero_fill_hotspots` ہر `metal` بینچ JSON۔

اس پلے بوک پر عمل کرکے ہم عین مطابق اپنانے کا مظاہرہ کرسکتے ہیں ، ایک فراہم کریں
ایک رول آؤٹ میں سنگل ثبوت کا بنڈل ، اور رول بیک ڈرل کے ساتھ ساتھ آڈٹ بھی رکھیں
دستخط شدہ بینچ مارک ظاہر ہوتا ہے۔