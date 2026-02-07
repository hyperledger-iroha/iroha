---
lang: ur
direction: rtl
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-04T10:50:53.613193+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! فاسٹ پی کیو پروڈکشن ہجرت گائیڈ

اس رن بک میں بتایا گیا ہے کہ اسٹیج 6 پروڈکشن فاسٹ پی کیو پروور کو کس طرح درست کیا جائے۔
اس ہجرت کے منصوبے کے ایک حصے کے طور پر پلے ہولڈر کا عین مطابق پسدید ہٹا دیا گیا تھا۔
یہ `docs/source/fastpq_plan.md` میں اسٹیج پلان کی تکمیل کرتا ہے اور فرض کرتا ہے کہ آپ پہلے ہی ٹریک کرتے ہیں
`status.md` میں ورک اسپیس کی حیثیت۔

## سامعین اور دائرہ کار
- ویلڈیٹر آپریٹرز اسٹیجنگ یا مینیٹ ماحول میں پروڈکشن پروور کو تیار کررہے ہیں۔
- بائنری یا کنٹینر بنانے والے انجینئرز کو جاری کریں جو پروڈکشن بیکینڈ کے ساتھ بھیج دیں گے۔
- SRE/مشاہدہ کرنے والی ٹیمیں ٹیلی میٹری کے نئے سگنلز کو وائرنگ کرتی ہیں اور انتباہ کرتی ہیں۔

دائرہ کار سے باہر: Kotodama معاہدہ تصنیف اور IVM ABI تبدیلیاں (`docs/source/nexus.md` دیکھیں
پھانسی کا ماڈل)۔

## فیچر میٹرکس
| راستہ | قابل بنانے کے لئے کارگو کی خصوصیات | نتیجہ | جب استعمال کریں |
| ---- | ------------------------- | ------ | ----------- |
| پروڈکشن پروور (پہلے سے طے شدہ) | _ کوئی_ | FFT/LDE پلانر اور گہری جمعہ پائپ لائن کے ساتھ STEGE6 FASTPQ پسدید۔ 【کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/بیکینڈ۔ آر ایس: 1144】 | تمام پروڈکشن بائنریز کے لئے پہلے سے طے شدہ۔ |
| اختیاری GPU ایکسلریشن | `fastpq_prover/fastpq-gpu` | خود کار طریقے سے سی پی یو فال بیک کے ساتھ CUDA/دھات کی دانا کو قابل بناتا ہے۔ 【کریٹس/فاسٹ پی کیو_پروور/کارگو.ٹومل: 9 】【 کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/ایف ایف ٹی آر آر ایس: 124】 | تعاون یافتہ ایکسلریٹرز کے ساتھ میزبان۔ |

## تعمیر کا طریقہ کار
1. ** سی پی یو صرف تعمیر **
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   پروڈکشن بیکینڈ کو بطور ڈیفالٹ مرتب کیا گیا ہے۔ کسی اضافی خصوصیات کی ضرورت نہیں ہے۔

2. ** GPU- قابل تعمیر (اختیاری) **
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   جی پی یو سپورٹ کے لئے ایک SM80+ CUDA ٹول کٹ کی ضرورت ہوتی ہے جس میں `nvcc` دستیاب ہے۔

3. ** خود ٹیسٹ **
   ```bash
   cargo test -p fastpq_prover
   ```
   پیکیجنگ سے پہلے اسٹیج 6 کے راستے کی تصدیق کے ل this اس ریلیز بلڈ کو ایک بار چلائیں۔

### دھاتی ٹول چین کی تیاری (میکوس)
1. عمارت سے پہلے میٹل کمانڈ لائن ٹولز انسٹال کریں: `xcode-select --install` (اگر CLI ٹولز غائب ہیں) اور GPU ٹولچین لانے کے لئے `xcodebuild -downloadComponent MetalToolchain`۔ بلڈ اسکرپٹ `xcrun metal`/`xcrun metallib` براہ راست اشارہ کرتا ہے اور اگر بائنریز غیر حاضر ہوں تو تیزی سے ناکام ہوجائے گی۔ 【کریٹ/فاسٹ پی کیو_پروور/بلڈ آر ایس: 98 】【 کریٹ/فاسٹ پی کیو_پروور/بلڈ۔ آر ایس: 121】
2. سی آئی سے پہلے پائپ لائن کو درست کرنے کے ل you ، آپ مقامی طور پر بلڈ اسکرپٹ کی عکسبندی کرسکتے ہیں:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   جب یہ کامیاب ہوجاتا ہے تو بلڈ ایمیٹس `FASTPQ_METAL_LIB=<path>` ؛ رن ٹائم میٹالیب کو تعی .ن سے لوڈ کرنے کے لئے اس قدر کو پڑھتا ہے۔
3. `FASTPQ_SKIP_GPU_BUILD=1` سیٹ کریں جب دھات کے ٹولچین کے بغیر کراس کمپل ہو۔ بلڈ ایک انتباہ پرنٹ کرتی ہے اور منصوبہ ساز سی پی یو کے راستے پر رہتا ہے۔ 【کریٹس/فاسٹ پی کیو_پروور/بلڈ۔
4. نوڈس خود بخود سی پی یو میں گر جاتے ہیں اگر دھات دستیاب نہیں ہے (گمشدہ فریم ورک ، غیر تعاون یافتہ جی پی یو ، یا خالی `FASTPQ_METAL_LIB`) ؛ بلڈ اسکرپٹ env var کو صاف کرتا ہے اور منصوبہ ساز ڈاؤن گریڈ کو لاگ ان کرتا ہے۔ 【کریٹس/فاسٹ پی کیو_پروور/بلڈ۔ آر ایس: 29 】【 کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/پس منظر۔### جاری چیک لسٹ (اسٹیج 6)
فاسٹ پی کیو کی رہائی کا ٹکٹ بلاک رکھیں جب تک کہ نیچے کی ہر شے مکمل اور منسلک نہ ہوجائے۔

1. ** سب سیکنڈ پروف میٹرکس **-تازہ پکڑے ہوئے `fastpq_metal_bench_*.json` کا معائنہ کریں اور
   `benchmarks.operations` اندراج کی تصدیق کریں جہاں `operation = "lde"` (اور آئینہ دار ہے
   `report.operations` نمونہ) 200000 کے کام کے بوجھ کے لئے `gpu_mean_ms ≤ 950` کی رپورٹیں (32768 پیڈڈ
   قطاریں)۔ چیک لسٹ پر دستخط کرنے سے پہلے چھت کے باہر کیپچروں کو دوبارہ دوبارہ کام کرنے کی ضرورت ہوتی ہے۔
2. ** دستخط شدہ مینی فیسٹ ** - چلائیں
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   لہذا ریلیز کا ٹکٹ مینی فیسٹ اور اس کے الگ الگ دستخط دونوں میں ہے
   (`artifacts/fastpq_bench_manifest.sig`)۔ جائزہ لینے والے پہلے ڈائجسٹ/دستخطی جوڑی کی تصدیق کرتے ہیں
   ایک ریلیز کو فروغ دینا
   `scripts/fastpq/capture_matrix.sh` کے ذریعے) پہلے ہی 20K قطار کا فرش اور انکوڈ کرتا ہے
   رجعت کو ڈیبگ کرنا۔
3.
   CUDA/دھات کے ظاہر ہونے والے نتائج ، اور ریلیز ٹکٹ پر علیحدہ دستخط۔ چیک لسٹ اندراج
   تمام نوادرات کے علاوہ عوامی کلیدی فنگر پرنٹ سے لنک کرنا چاہئے جس پر دستخط کرنے کے لئے استعمال کیا جاتا ہے تو بہاو آڈٹ
   توثیق کے مرحلے کو دوبارہ چلا سکتے ہیں۔### دھات کی توثیق کا ورک فلو
1. جی پی یو سے چلنے والی تعمیر کے بعد ، `.metallib` (`echo $FASTPQ_METAL_LIB`) پر `FASTPQ_METAL_LIB` پوائنٹس کی تصدیق کریں تاکہ رن ٹائم اسے طے شدہ طور پر لوڈ کرسکے۔ 【کریٹس/فاسٹ پی کیو_پروور/بلڈ آر ایس: 188】
2. جی پی یو لینوں کے ساتھ پیرٹی سویٹ چلائیں: \
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`۔ پسدید دھات کی دانا کا استعمال کرے گا اور اگر پتہ لگانے میں ناکام ہوجاتا ہے تو ایک عین مطابق سی پی یو فال بیک لاگ ان کریں گے۔
3. ڈیش بورڈز کے لئے ایک بینچ مارک نمونہ پر قبضہ کریں: \
   مرتب شدہ میٹل لائبریری (`fd -g 'fastpq.metallib' target/release/build | head -n1`) کا پتہ لگائیں ،
   اسے `FASTPQ_METAL_LIB` کے ذریعے برآمد کریں ، اور چلائیں \
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`۔
  کیننیکل `fastpq-lane-balanced` پروفائل اب ہر کیپچر کو 32،768 قطاروں (2⁵) پر پیڈ کرتا ہے ، لہذا JSON دھاتی LDE لیٹینسی کے ساتھ ساتھ `rows` اور `padded_rows` بھی رکھتا ہے۔ اگر `zero_fill` یا قطار کی ترتیبات GPU LDE کو 950ms (<1s) کے ہدف سے آگے ایپلیم سیریز کے میزبانوں پر دھکیلیں تو گرفتاری کو دوبارہ بنائیں۔ دیگر ریلیز شواہد کے ساتھ ساتھ JSON/لاگ کو محفوظ شدہ دستاویزات۔ رات کے وقت میکوس ورک فلو ایک ہی رن کو انجام دیتا ہے اور موازنہ کے ل its اپنے نوادرات کو اپ لوڈ کرتا ہے۔ 【کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/ایس آر سی/بن/فاسپ کیو_میٹل_بینچ۔
  جب آپ کو پوسیڈن صرف ٹیلی میٹری کی ضرورت ہو (جیسے ، آلات کا سراغ ریکارڈ کرنے کے لئے) ، `--operation poseidon_hash_columns` کو مذکورہ کمانڈ میں شامل کریں۔ بینچ اب بھی `FASTPQ_GPU=gpu` کا احترام کرے گا ، `metal_dispatch_queue.poseidon` کا اخراج کرے گا ، اور نیا `poseidon_profiles` بلاک شامل کرے گا لہذا ریلیز کے بنڈل نے پوسیڈن کی رکاوٹ کو واضح طور پر دستاویز کیا ہے۔
  شواہد میں اب `zero_fill.{bytes,ms,queue_delta}` پلس `kernel_profiles` (فی کارنل شامل ہیں
  قبضہ ، تخمینہ جی بی/ایس ، اور مدت کے اعدادوشمار) لہذا جی پی یو کی کارکردگی کو بغیر گراف کیا جاسکتا ہے
  خام نشانات کو دوبارہ تیار کرنا ، اور ایک `twiddle_cache` بلاک (ہٹ/یاد کرتا ہے + `before_ms`/`after_ms`)
  ثابت کرتا ہے کہ کیچڈ سگلی اپ لوڈز نافذ ہیں۔ `--trace-dir` کے تحت استعمال کو دوبارہ لانچ کرتا ہے
  `xcrun xctrace record` اور
  JSON کے ساتھ ساتھ ایک ٹائم اسٹیمپڈ `.trace` فائل اسٹور کرتا ہے۔ آپ اب بھی بیسوکی فراہم کرسکتے ہیں
  `--trace-output` (اختیاری `--trace-template` / `--trace-seconds` کے ساتھ)
  کسٹم مقام/ٹیمپلیٹ۔ JSON آڈٹ کے لئے `metal_trace_{template,seconds,output}` ریکارڈ کرتا ہے۔ہر کیپچر کے بعد `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` ہے لہذا اشاعت میں میزبان میٹا ڈیٹا (جس میں اب `metadata.metal_trace` بھی شامل ہے) Grafana بورڈ/الرٹنگ بنڈل (`dashboards/grafana/fastpq_acceleration.json` ، `dashboards/alerts/fastpq_acceleration_rules.yml`) کے لئے میزبان میٹا ڈیٹا (`metadata.metal_trace` بھی شامل ہے)۔ اس رپورٹ میں اب ایک آپریشن (`speedup.ratio` ، `speedup.delta_ms`) ، ریپر ہائٹس `zero_fill_hotspots` (بائٹس ، لیٹینسی ، حاصل کردہ جی بی/ایس ، اور دھاتی قطار ڈیلٹا کاؤنٹرز) ، اور دھاتی قطار ڈیلٹا کاؤنٹرز) Prometheus ، اور میٹل قطار ڈیلٹا کاؤنٹرز ، فلیٹینز `speedup.ratio`) ہے۔ `twiddle_cache` بلاک کو برقرار رکھتا ہے ، نیا `post_tile_dispatches` بلاک/سمری کی کاپی کرتا ہے تاکہ جائزہ لینے والے اس گرفتاری کے دوران ملٹی پاس کی دانا کو ثابت کرسکیں ، اور اب پوسیڈن مائکروبینچ شواہد کا خلاصہ `benchmarks.poseidon_microbench` کے بغیر رپیسڈس میں کیا جاسکتا ہے۔ منشور گیٹ ایک ہی بلاک کو پڑھتا ہے اور جی پی یو شواہد کے بنڈلوں کو مسترد کرتا ہے جو اسے چھوڑ دیتے ہیں ، جب بھی ٹائلنگ کے بعد کے راستے کو چھوڑنے کے بعد آپریٹرز کو ریفریش کرنے پر مجبور کرتے ہیں یا غلط کنفیگرڈ۔ 【کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/بن/فاسپ کیو_میٹل_بنچ۔
  پوسیڈون 2 دھات کی دانا ایک ہی نوبس کا اشتراک کرتی ہے: `FASTPQ_METAL_POSEIDON_LANES` (32–256 ، دو کے اختیارات) اور `FASTPQ_METAL_POSEIDON_BATCH` (1–32 ریاستیں فی لین) آپ کو بغیر کسی تعمیر نو کے لانچ کی چوڑائی اور فی لین کام کو پن کرنے دیں۔ میزبان ان اقدار کو ہر بھیجنے سے پہلے `PoseidonArgs` کے ذریعے تھریڈ کرتا ہے۔ پہلے سے طے شدہ رن ٹائم `MTLDevice::{is_low_power,is_headless,location}` کا معائنہ کرتا ہے تاکہ VRAM-tired لانچوں (`256×24` جب ≥48gib کی اطلاع دی جاتی ہے ، `256×20` میں 32GIB ، `256×16` دوسری صورت میں) کی طرف اشارہ کیا جاتا ہے۔ 8/6 ریاستوں میں فی لین) ، لہذا زیادہ تر آپریٹرز کو کبھی بھی دستی طور پر env vars سیٹ کرنے کی ضرورت نہیں ہے۔ 【کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/میٹل_کونفگ۔ `poseidon_microbench` بلاک جس میں دونوں لانچ پروفائلز کے علاوہ پیمائش شدہ اسپیڈ اپ کو ریکارڈ کرتا ہے اس کے مقابلے میں اسکیلر لین کے مقابلے میں ریلیز بنڈل یہ ثابت کر سکتے ہیں کہ نئی دانا حقیقت میں `poseidon_hash_columns` کو سکڑ سکتا ہے ، اور اس میں `poseidon_pipeline` بلاک شامل ہے لہذا اسٹیج 7 شواہد کو نئے حصے میں شامل کرنے کے ساتھ ساتھ نئے سرکجنٹ کو شامل کیا گیا ہے۔ عام رنز کے لئے env unset چھوڑ دیں۔ کنٹرول دوبارہ عملدرآمد کا انتظام کرتا ہے ، اگر بچوں کی گرفتاری نہیں چل سکتی ہے تو ناکامیوں کو لاگ ان کرتا ہے ، اور جب `FASTPQ_GPU=gpu` سیٹ کیا جاتا ہے تو فوری طور پر باہر نکل جاتا ہے لیکن کوئی GPU پسدید دستیاب نہیں ہے لہذا خاموش سی پی یو فال بیکس کبھی بھی پرفیٹ میں چپکے چپکے نہیں چھپ جاتے ہیں۔ نوادرات۔ریپر نے پوسیڈن کی گرفت کو مسترد کردیا جو `metal_dispatch_queue.poseidon` ڈیلٹا ، مشترکہ `column_staging` کاؤنٹرز ، یا `poseidon_profiles`/`poseidon_microbench` بلاکس کے بلاکس سے محروم ہیں لہذا آپریٹرز کو کسی بھی طرح کی گرفتاری کو تازہ کرنے میں ناکام ہوجاتا ہے جو اوورلیپنگ اسٹیج کو ثابت کرنے میں ناکام ہوجاتا ہے۔ اسپیڈ اپ۔ 【اسکرپٹس/فاسٹ پی کیو/لپیٹ_بنچ مارک۔ پی وائی: 732】 جب آپ کو ڈیش بورڈز یا سی آئی ڈیلٹا کے لئے اسٹینڈ لون JSON کی ضرورت ہوتی ہے تو ، `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` چلائیں ؛ مددگار دونوں کو لپیٹے ہوئے نمونے اور را `fastpq_metal_bench*.json` کیپچرز کو قبول کرتا ہے ، جو پہلے سے طے شدہ/اسکیلر اوقات کے ساتھ `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` کو خارج کرتا ہے ، ٹیوننگ میٹا ڈیٹا ، اور ریکارڈ شدہ اسپیڈ اپ۔
  `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` پر عمل درآمد کرکے رن کو ختم کریں لہذا اسٹیج 6 کی رہائی کی فہرست `<1 s` LDE چھت کو نافذ کرتی ہے اور ایک دستخط شدہ مینی فیسٹ/ڈائجسٹ بنڈل خارج کرتی ہے جو ریلیز ٹکٹ کے ساتھ جہاز بھیجتی ہے۔
4. رول آؤٹ سے پہلے ٹیلی میٹری کی تصدیق کریں: Prometheus اختتامی نقطہ (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) کو curl کریں اور غیر متوقع `resolved="cpu"` کے لئے `telemetry::fastpq.execution_mode` لاگز کا معائنہ کریں۔ اندراجات۔ 【کریٹس/آئروہ_ٹیلمیٹری/ایس آر سی/میٹرکس۔
5. سی پی یو فال بیک بیک کو جان بوجھ کر مجبور کرکے دستاویز کریں (`FASTPQ_GPU=cpu` یا `zk.fastpq.execution_mode = "cpu"`) لہذا SRE پلے بوکس کو عزم سلوک کے ساتھ منسلک رکھا جاتا ہے۔6. اختیاری ٹیوننگ: پہلے سے طے شدہ طور پر میزبان مختصر نشانات کے لئے 16 لین ، میڈیم کے لئے 32 ، اور 64/128 ایک بار `log_len ≥ 10/14` کا انتخاب کرتا ہے ، جب `log_len ≥ 18` پر 256 پر اترتا ہے ، اور اب یہ چھوٹے ٹریس کے لئے پانچ مراحل پر مشترکہ میموری ٹائل ، چار ایک بار IVM/1614 میں رکھتا ہے۔ `log_len ≥ 18/20/22` پوسٹ ٹائلنگ کے بعد کے دانا پر لات مارنے سے پہلے۔ ان ہورسٹکس کو اوور رائڈ کرنے کے لئے اوپر والے اقدامات کو چلانے سے پہلے `FASTPQ_METAL_FFT_LANES` (8AND256 کے درمیان پاور آف ٹو) اور/یا `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) برآمد کریں۔ دونوں ایف ایف ٹی/آئی ایف ایف ٹی اور ایل ڈی ای کالم بیچ کے سائز حل شدہ تھریڈ گروپ کی چوڑائی (≈2048 منطقی دھاگے فی ڈسپیچ ، 32 کالموں پر ڈھکے ہوئے ہیں ، اور اب 32 → 16 → 8 → 8 → 4 → 4 → 2 → 1 کے ذریعے ڈومین کیپ میں اضافے کے ساتھ ہی اس کے ڈومین کیپٹ پر عمل کرتے ہیں۔ جب آپ کو میزبانوں میں بٹ فار بٹ موازنہ کی ضرورت ہوتی ہے تو ایل ڈی ای ڈسپیچر کو اسی اوور رائڈ کا اطلاق کرنے کے لئے ایک عین مطابق ایف ایف ٹی بیچ سائز اور `FASTPQ_METAL_LDE_COLUMNS` (1–32) کو پن کرنے کے لئے `FASTPQ_METAL_FFT_COLUMNS` (1–32) سیٹ کریں۔ ایل ڈی ٹائل کی گہرائی ایف ایف ٹی ہیورسٹکس کے ساتھ ساتھ آئی 18 این آئی00000112x کے ساتھ بھی ٹریس صرف 12/10/8 مشترکہ میموری کے مراحل کو ٹائلنگ کے بعد کے دانا کو وسیع تتلیوں کے حوالے کرنے سے پہلے چلتی ہے۔ رن ٹائم دھات کے دانا آرگس کے ذریعے تمام اقدار کو دھاگے میں ڈالتا ہے ، غیر تعاون یافتہ کلیمپوں کو ختم کرتا ہے ، اور حل شدہ اقدار کو لاگ کرتا ہے تاکہ تجربات میٹالیب کی تعمیر نو کے بغیر تولیدی رہیں۔ ایل ڈی ای کے اعدادوشمار کے ذریعہ پکڑے گئے حل شدہ ٹیوننگ اور میزبان صفر سے بھرنے والے بجٹ (`zero_fill.{bytes,ms,queue_delta}`) دونوں کو حل کرنے والے ٹننگ اور میزبان صفر سے بھرنے والے بجٹ (`zero_fill.{bytes,ms,queue_delta}`) دونوں کی سطح پر روشنی ڈیلٹاس کو براہ راست ہر گرفتاری سے جوڑ دیا جاتا ہے ، اور اب `column_staging` بلاک کے ذریعہ ORDER کے ذریعہ (BATACHS فلیٹین_مز ، ویٹ_مز ، ویٹ_مز ، انتظار کریں ، ڈبل بفرڈ پائپ لائن۔ جب جی پی یو صفر سے بھرنے والے ٹیلی میٹری کی اطلاع دینے سے انکار کرتا ہے تو ، اب اس کا استعمال میزبان سائیڈ بفر سے ایک اختیاری وقت کی ترکیب کرتا ہے اور اسے `zero_fill` بلاک میں صاف کرتا ہے اور انجکشن لگاتا ہے لہذا شواہد کو جاری نہیں کرتے ہیں۔ فیلڈ۔ 【کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/میٹل_کونفگ al_bench.rs:575】【crates/fastpq_prover/src/bin/fastpq_metal_binch.rs:1609】【crates/fastpq_prover/src/bin/fastpq_metal_binch.rs:1860】7. ملٹی کیو ڈسپیچ مجرد میکس پر خودکار ہے: جب `Device::is_low_power()` غلط کو واپس کرتا ہے یا دھات کا آلہ ایک سلاٹ/بیرونی جگہ کی اطلاع دیتا ہے تو میزبان دو `MTLCommandQueue`s کو روکنے کے بغیر رہتا ہے ، صرف ایک بار جب کام کے بوجھ کے ساتھ ہی شائقین quignes kiseds quetness ، اور گول رابنز کے ذریعہ راؤنڈ راؤنڈ روبنز بنائے جاتے ہیں) سمجھوتہ کرنا عزم۔ `FASTPQ_METAL_QUEUE_FANOUT` (1–4 قطاریں) اور `FASTPQ_METAL_COLUMN_THRESHOLD` (فین آؤٹ سے پہلے کم سے کم کل کالم) کے ساتھ پالیسی کو اوور رائڈ کریں جب بھی آپ کو مشینوں میں تولیدی کیپچر کی ضرورت ہو۔ برابری کے ٹیسٹ ان اوور رائڈس کو مجبور کرتے ہیں تاکہ ملٹی جی پی یو میکس کا احاطہ کیا جائے اور حل شدہ فین آؤٹ/تھریشولڈ قطار کی گہرائی کے ساتھ ہی لاگ ان ہوں۔ ٹیلی میٹری۔ 【کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/دھات۔آرکائیو کے لئے ### ثبوت
| نوادرات | گرفت | نوٹ |
| ---------- | --------- | ------- |
| `.metallib` بنڈل | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` اور `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` کے بعد `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` اور `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib` کے بعد۔ | یہ ثابت کرتا ہے کہ دھات کی سی ایل آئی/ٹولچین انسٹال کی گئی تھی اور اس کمٹ کے لئے ایک ڈٹرمینسٹک لائبریری تیار کی گئی تھی۔ 【کریٹس/فاسٹ پی کیو_پروور/بلڈ آر آر ایس: 98 】【 کریٹس/فاسٹ پی کیو_پروور/بلڈ آر آر ایس: 188】 |
| ماحولیات سنیپ شاٹ | `echo $FASTPQ_METAL_LIB` تعمیر کے بعد ؛ اپنی رہائی کے ٹکٹ کے ساتھ مطلق راستہ رکھیں۔ | خالی آؤٹ پٹ کا مطلب ہے دھات غیر فعال تھی۔ قیمت کے دستاویزات کی ریکارڈنگ جو جی پی یو لین شپنگ آرٹ فیکٹ پر دستیاب ہیں۔
| GPU parity لاگ | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` اور اسنیپٹ کو محفوظ شدہ دستاویزات جس میں `backend="metal"` یا ڈاؤن گریڈ انتباہ ہوتا ہے۔ | یہ ظاہر کرتا ہے کہ آپ کی تعمیر کو فروغ دینے سے پہلے ہی دانا (یا عین مطابق انداز میں واپس آجاتا ہے)۔
| بینچ مارک آؤٹ پٹ | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces` ؛ `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]` کے ذریعے لپیٹ اور سائن کریں۔ | لپیٹ شدہ JSON ریکارڈ `speedup.ratio` ، `speedup.delta_ms` ، FFT ٹیوننگ ، پیڈڈ قطاریں (32،768) ، افزودہ `zero_fill`/`kernel_profiles` ، فلیٹڈ `kernel_summary` ، تصدیق شدہ `metal_dispatch_queue.poseidon`/`poseidon_profiles` بلاکس (جب `--operation poseidon_hash_columns` استعمال ہوتا ہے) ، اور ٹریس میٹا ڈیٹا لہذا GPU LDE کا مطلب ہے ≤950ms اور پوسیڈن قیام <1s ؛ ریلیز ٹکٹ کے ساتھ بنڈل اور پیدا شدہ `.json.asc` دستخط دونوں رکھیں تاکہ ڈیش بورڈز اور آڈیٹر بغیر کسی رننگ کے نوادرات کی تصدیق کرسکیں کام کے بوجھ۔ 【کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/بن/فاسپ کیو_میٹل_بنچ
| بینچ منشور | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`۔ | دونوں جی پی یو نوادرات کی توثیق کرتا ہے ، اگر ایل ڈی ای کا مطلب `<1 s` چھت کو توڑ دیتا ہے تو ، بلیک 3/SHA-256 ڈائجسٹس کو ریکارڈ کرتا ہے ، اور دستخط شدہ مظہر کو خارج کرتا ہے لہذا ریلیز چیک لسٹ قابل تصدیق کے بغیر آگے نہیں بڑھ سکتی میٹرکس۔ 【ایکس ٹی ایس اے ایس سی/ایس آر سی/فاسٹ پی کیو آر ایس: 1 】【 نمونے/فاسٹ پی کیو_بنچمارکس/ریڈیمیم ڈاٹ ایم ڈی: 65】 |
| CUDA بنڈل | `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` کو SM80 لیب ہوسٹ پر چلائیں ، JSON کو `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` میں لپیٹ/سائن کریں (`--label device_class=xeon-rtx-sm80` استعمال کریں لہذا ڈیش بورڈز صحیح کلاس کو منتخب کریں) ، `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` کے ساتھ جوڑا شامل کریں ، اور `.json`/Prometheus کو رکھیں۔ منشور چیکڈ ان `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` عین مطابق بنڈل فارمیٹ آڈیٹرز کی توقع کرتا ہے۔
| ٹیلی میٹری پروف | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` پلس `telemetry::fastpq.execution_mode` لاگ ان اسٹارٹ اپ پر خارج ہوتا ہے۔ | ٹریفک کو چالو کرنے سے پہلے Prometheus/OTEL `device_class="<matrix>", backend="metal"` (یا ایک ڈاؤن گریڈ لاگ) کو بے نقاب کریں۔| جبری سی پی یو ڈرل | `FASTPQ_GPU=cpu` یا `zk.fastpq.execution_mode = "cpu"` کے ساتھ ایک مختصر بیچ چلائیں اور ڈاون گریڈ لاگ کو پکڑیں۔ | ایس آر ای رن بوکس کو درمیانی ریلیز کی ضرورت ہونے کی صورت میں ایک رول بیک بیک بیک راہ کے ساتھ منسلک رکھی جاتی ہے۔
| ٹریس کیپچر (اختیاری) | `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` کے ساتھ برابری کے ٹیسٹ کو دہرائیں اور خارج ہونے والے ڈسپیچ ٹریس کو بچائیں۔ | بعد میں دوبارہ بینچ مارک کے بغیر پروفائلنگ جائزوں کے لئے قبضہ/تھریڈ گروپ کے ثبوت کو محفوظ رکھتا ہے۔

کثیر لسانی `fastpq_plan.*` فائلوں کا حوالہ یہ چیک لسٹ ہے لہذا اسٹیجنگ اور پروڈکشن آپریٹرز ایک ہی ثبوت کی پگڈنڈی پر عمل کرتے ہیں۔

## تولیدی تعمیرات
تولیدی اسٹیج 6 نوادرات پیدا کرنے کے لئے پنڈ کنٹینر ورک فلو کا استعمال کریں:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

مددگار اسکرپٹ `rust:1.88.0-slim-bookworm` ٹولچین امیج (اور GPU کے لئے `nvidia/cuda:12.2.2-devel-ubuntu22.04`) تیار کرتا ہے ، کنٹینر کے اندر تعمیر چلاتا ہے ، اور `manifest.json` ، `sha256s.txt` ، اور مرتب شدہ بائنریوں کو ہدف آؤٹ پٹ کے لئے لکھتا ہے۔ ڈائرکٹری۔ 【اسکرپٹ/فاسٹ پی کیو/ریپرو_بولڈ.ش: 1 】【 اسکرپٹ/فاسٹ پی کیو/رن_نسائڈ_رپرو_بولڈ.ش: 1 】【 اسکرپٹ/فاسٹ پی کیو/ڈاکر/ڈاکو فائل. جی پی یو: 1】

ماحولیات کو ختم کرنا:
- `FASTPQ_RUST_IMAGE` ، `FASTPQ_RUST_TOOLCHAIN` - ایک واضح مورچا بیس/ٹیگ پر پن کریں۔
- `FASTPQ_CUDA_IMAGE` - GPU نوادرات تیار کرتے وقت CUDA بیس کو تبدیل کریں۔
- `FASTPQ_CONTAINER_RUNTIME` - ایک مخصوص رن ٹائم پر مجبور کریں۔ پہلے سے طے شدہ `auto` `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` کی کوشش کرتا ہے۔
-`FASTPQ_CONTAINER_RUNTIME_FALLBACKS`-رن ٹائم آٹو ڈٹیکشن کے لئے کوما سے الگ ترجیحی آرڈر (`docker,podman,nerdctl` میں پہلے سے طے شدہ)۔

## کنفیگریشن اپڈیٹس
1. اپنے ٹومل میں رن ٹائم ایگزیکیوشن موڈ سیٹ کریں:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   قیمت `FastpqExecutionMode` اور تھریڈز کے ذریعے شروع کی جاتی ہے۔

2. ضرورت پڑنے پر لانچ کے وقت اوور رائڈ:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   سی ایل آئی اوور رائڈس نوڈ کے جوتے سے پہلے حل شدہ کنفگ کو تبدیل کریں۔

3. ڈویلپرز برآمد کرکے کنفیگس کو چھوئے بغیر عارضی طور پر پتہ لگانے پر مجبور کرسکتے ہیں
   بائنری لانچ کرنے سے پہلے `FASTPQ_GPU={auto,cpu,gpu}` ؛ اوور رائڈ لاگ ان اور پائپ لائن ہے
   پھر بھی حل شدہ وضع کی سطح پر ہے۔ 【کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/بیکینڈ۔

## توثیق چیک لسٹ
1. ** اسٹارٹ اپ لاگ **
   - `FASTPQ execution mode resolved` کے ساتھ `telemetry::fastpq.execution_mode` سے `FASTPQ execution mode resolved` کی توقع کریں
     `requested` ، `resolved` ، اور `backend` لیبل۔
   - خود کار طریقے سے جی پی یو کا پتہ لگانے پر `fastpq::planner` سے ایک ثانوی لاگ حتمی لین کی اطلاع دیتا ہے۔
   - دھات کی میزبان سطح `backend="metal"` جب میٹالیب کامیابی کے ساتھ بوجھ ڈالتا ہے۔ اگر تالیف یا لوڈنگ میں ناکام ہوجاتا ہے تو بلڈ اسکرپٹ ایک انتباہ کا اخراج کرتا ہے ، `FASTPQ_METAL_LIB` کو صاف کرتا ہے ، اور منصوبہ ساز `GPU acceleration unavailable` کو جاری رکھنے سے پہلے ریکارڈ کرتا ہے سی پی یو۔ 【کریٹس/فاسٹ پی کیو_پروور/بلڈ آر آر ایس: 29 】【 کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/بیکینڈ۔ آر ایس: 174 】【 کریٹس/فاسٹ پی کیو_پروور/ایس آر سی/بیکینڈ۔2. ** Prometheus میٹرکس **
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   کاؤنٹر کو `record_fastpq_execution_mode` کے ذریعے بڑھایا جاتا ہے (اب لیبل لگا ہوا ہے
   `{device_class,chip_family,gpu_kind}`) جب بھی کوئی نوڈ اس کی پھانسی کو حل کرتا ہے
   موڈ. 【کریٹس/اروہ_ٹیلمیٹری/ایس آر سی/میٹرکس۔ آر ایس: 8887】
   - دھات کی کوریج کی تصدیق کے لئے
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     آپ کے تعیناتی ڈیش بورڈز کے ساتھ ساتھ اضافہ۔
   - میکوس نوڈس `irohad --features fastpq-gpu` کے ساتھ مرتب کردہ اضافی طور پر بے نقاب
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     اور
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` لہذا اسٹیج 7 ڈیش بورڈز
     براہ راست Prometheus سکریپس سے ڈیوٹی سائیکل اور قطار کے ہیڈ روم کو ٹریک کرسکتے ہیں۔ 【کریٹس/اروہہ_ٹیلمیٹری/ایس آر سی/میٹرکس۔

3. ** ٹیلی میٹری ایکسپورٹ **
   - اوٹیل ایک ہی لیبلوں کے ساتھ `fastpq.execution_mode_resolutions_total` emit تیار کرتا ہے۔ اپنے کو یقینی بنائیں
     جب GPUs فعال ہونا چاہئے جب غیر متوقع `resolved="cpu"` کے لئے ڈیش بورڈز یا الرٹس دیکھیں۔

4. ** سنجیدگی ثابت/تصدیق **
   - `iroha_cli` کے ذریعے ایک چھوٹا سا بیچ چلائیں یا ایک انضمام کا استعمال کریں اور تصدیق کے ثبوتوں کی تصدیق a پر کریں
     ہم مرتبہ ایک ہی پیرامیٹرز کے ساتھ مرتب کیا گیا ہے۔

## خرابیوں کا سراغ لگانا
- ** حل شدہ موڈ جی پی یو میزبانوں پر سی پی یو رہتا ہے ** - چیک کریں کہ بائنری کے ساتھ بنایا گیا تھا
  `fastpq_prover/fastpq-gpu` ، CUDA لائبریریاں لوڈر کے راستے پر ہیں ، اور `FASTPQ_GPU` مجبور نہیں ہے
  `cpu`۔
۔ ایک خالی یا گمشدہ قیمت ڈیزائن کے ذریعہ پسدید کو غیر فعال کردیتی ہے۔
- ** `Unknown parameter` غلطیاں ** - یقینی بنائیں کہ پروور اور تصدیق کنندہ دونوں ایک ہی کیننیکل کیٹلاگ کا استعمال کریں
  `fastpq_isi` کے ذریعہ خارج ؛ `Error::UnknownParameter` کے طور پر مماثل سطح
- ** غیر متوقع سی پی یو فال بیک ** - `cargo tree -p fastpq_prover --features` کا معائنہ کریں اور
  تصدیق کریں `fastpq_prover/fastpq-gpu` GPU بلڈز میں موجود ہے۔ تصدیق کریں `nvcc`/CUDA لائبریریاں تلاش کے راستے پر ہیں۔
- ** ٹیلی میٹری کاؤنٹر لاپتہ ** - تصدیق کریں نوڈ `--features telemetry` (پہلے سے طے شدہ) کے ساتھ شروع کیا گیا تھا
  اور یہ کہ اوٹیل ایکسپورٹ (اگر فعال ہے) میں میٹرک پائپ لائن شامل ہے۔

## فال بیک کا طریقہ کار
ڈٹرمینسٹک پلیس ہولڈر بیکینڈ کو ہٹا دیا گیا ہے۔ اگر کسی رجعت کو رول بیک کی ضرورت ہوتی ہے ،
پہلے سے جانا جاتا اچھے رہائی کے نوادرات کو دوبارہ پیش کریں اور اسٹیج 6 کو دوبارہ جاری کرنے سے پہلے تفتیش کریں
بائنریز تبدیلی کے انتظام کے فیصلے کی دستاویز کریں اور یقینی بنائیں کہ فارورڈ رول صرف اس کے بعد ہی مکمل ہوجائے
رجعت سمجھا جاتا ہے۔

3. `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` کو یقینی بنانے کے لئے ٹیلی میٹری کی نگرانی کریں توقع کی عکاسی کرتی ہے
   پلیس ہولڈر پر عمل درآمد۔

## ہارڈ ویئر بیس لائن
| پروفائل | سی پی یو | GPU | نوٹ |
| ------- | --- | --- | ----- |
| حوالہ (اسٹیج 6) | AMD EPEC7B12 (32 کور) ، 256GIB رام | Nvidia A10040GB (CUDA12.2) | 20000 قطار مصنوعی بیچوں کو ≤1000ms کو مکمل کرنا چاہئے۔ 【دستاویزات/ماخذ/فاسٹ پی کیو_پلان
| CPU- صرف | ≥32 جسمانی کور ، AVX2 | - | 20000 قطاروں کے لئے ~ 0.9–1.2s کی توقع کریں۔ عزم کے ل I `execution_mode = "cpu"` رکھیں۔ |## رجعت پسندی ٹیسٹ
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (GPU میزبانوں پر)
- اختیاری گولڈن فکسچر چیک:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

اس چیک لسٹ سے کسی بھی انحراف کو اپنی اوپس رن بک میں دستاویز کریں اور اس کے بعد `status.md` کو اپ ڈیٹ کریں
ہجرت کی ونڈو مکمل ہوتی ہے۔