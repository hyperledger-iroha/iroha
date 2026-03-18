---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-Rollout-plan
عنوان: SNNET-16G پوسٹ کوانٹم تعیناتی کا منصوبہ
سائڈبار_لیبل: پی کیو تعیناتی منصوبہ
تفصیل: سورانیٹ کے X25519+ML-KEM ہائبرڈ ہینڈ شیک کو کینری سے ڈیفالٹ تک ریلے ، کلائنٹ اور ایس ڈی کے میں فروغ دینے کے لئے آپریشنل گائیڈ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/soranet/pq_rollout_plan.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں۔
:::

Snnet-16G سورنیٹ ٹرانسپورٹ کے لئے پوسٹ کوانٹم کی تعیناتی کو مکمل کرتا ہے۔ `rollout_phase` کنٹرولز آپریٹرز کو اسٹیج A کی موجودہ گارڈ کی ضرورت سے اسٹیج بی کی اکثریت کوریج اور ہر سطح کے لئے خام JSON/ٹومل میں ترمیم کیے بغیر اسٹیج سی کی سخت پی کیو کرنسی کو مربوط کرنے کی اجازت دیتے ہیں۔

اس پلے بک کا احاطہ کرتا ہے:

- مرحلے کی تعریفیں اور نئی ترتیب knobs (`sorafs.gateway.rollout_phase` ، `sorafs.rollout_phase`) کوڈ بیس (`crates/iroha_config/src/parameters/actual.rs:2230` ، `crates/iroha/src/config/user.rs:251`) میں وائرڈ۔
- ایس ڈی کے اور سی ایل آئی کے جھنڈوں کی نقشہ سازی تاکہ ہر کلائنٹ رول آؤٹ کی پیروی کرسکے۔
- کینری ریلے/کلائنٹ کی شیڈولنگ توقعات کے علاوہ گورننس ڈیش بورڈز جو فروغ (`dashboards/grafana/soranet_pq_ratchet.json`) کو ٹریک کرتے ہیں۔
- رول بیک ہکس اور فائر ڈرل رن بک ([پی کیو رچیٹ رن بک] (./pq-ratchet-runbook.md)) کے حوالہ جات۔

## فیز کا نقشہ

| `rollout_phase` | موثر گمنامی کا مرحلہ | پہلے سے طے شدہ اثر | عام استعمال |
| ------------------- | -------------------------------------------------------- | --------------- |
| `canary` | `anon-guard-pq` (اسٹیج A) | کم از کم ایک پی کیو گارڈ فی سرکٹ کی ضرورت ہوتی ہے جبکہ بیڑے گرم ہوجاتے ہیں۔ | بیس لائن اور کینری کے پہلے ہفتوں۔ |
| `ramp` | `anon-majority-pq` (اسٹیج B) | > = دو تہائی کوریج کے لئے پی کیو ریلے کی طرف تعصب کا انتخاب ؛ کلاسیکی ریلے فال بیک کے طور پر باقی ہے۔ | ریلے خطے کے ذریعہ کینری ؛ پیش نظارہ SDK میں ٹوگل۔ |
| `default` | `anon-strict-pq` (اسٹیج سی) | صرف پی کیو صرف سرکٹس لگائیں اور ہارڈن ڈاون گریڈ الارمز لگائیں۔ | ایک بار ٹیلی میٹری اور گورننس سائن آف مکمل ہونے کے بعد حتمی تشہیر۔ |

اگر کوئی سطح بھی ایک واضح `anonymity_policy` کی وضاحت کرتی ہے تو ، اس جزو کے لئے اس مرحلے کو اوور رائڈ کرتا ہے۔ واضح مرحلے کو چھوڑیں اب `rollout_phase` کی قیمت سے ہٹ جاتا ہے تاکہ آپریٹرز ہر ماحول میں صرف ایک بار اسٹیج کو تبدیل کرسکیں اور مؤکلوں کو اس کا وارث ہونے دیں۔

## ترتیب حوالہ

### آرکسٹیٹر (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

آرکیسٹریٹر لوڈر رن ٹائم (`crates/sorafs_orchestrator/src/lib.rs:2229`) میں فال بیک مرحلے کو حل کرتا ہے اور اسے `sorafs_orchestrator_policy_events_total` اور `sorafs_orchestrator_pq_ratio_*` کے ذریعے بے نقاب کرتا ہے۔ `docs/examples/sorafs_rollout_stage_b.toml` اور `docs/examples/sorafs_rollout_stage_c.toml` دیکھیں جس میں ٹکڑوں کو درخواست دینے کے لئے تیار ہے۔

### مورچا کلائنٹ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` اب پارسڈ مرحلے (`crates/iroha/src/client.rs:2315`) کو ریکارڈ کرتا ہے تاکہ مددگار (مثال کے طور پر `iroha_cli app sorafs fetch`) پہلے سے طے شدہ گمنامی کی پالیسی کے ساتھ موجودہ مرحلے کی اطلاع دے سکے۔

## آٹومیشن

دو `cargo xtask` مددگار شیڈول کی نسل اور نمونے کی گرفتاری کو خود کار بناتے ہیں۔

1. ** علاقائی شیڈول پیدا کریں **

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```دورانیے لاحقہ قبول کریں کمانڈ `artifacts/soranet_pq_rollout_plan.json` اور مارک ڈاون سمری (`artifacts/soranet_pq_rollout_plan.md`) کو آؤٹ پٹ کرتا ہے جو تبدیلی کی درخواست کے ساتھ بھیجا جاسکتا ہے۔

2. ** دستخطوں کے ساتھ ڈرل نمونے کی گرفتاری **

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   کمانڈ فراہم کردہ فائلوں کو `artifacts/soranet_pq_rollout/<timestamp>_<label>/` پر کاپی کرتی ہے ، ہر نمونے کے لئے بلیک 3 ڈائجسٹوں کا حساب لگاتی ہے اور میٹا ڈیٹا کے ساتھ `rollout_capture.json` لکھتی ہے اور ادائیگی کے بوجھ پر ED25519 دستخط کے ساتھ۔ وہی نجی کلید استعمال کریں جو فائر ڈرل منٹ پر دستخط کرتی ہے تاکہ گورننس اس گرفتاری کو جلد توثیق کرسکے۔

## SDK اور CLI جھنڈوں کی سرنی

| سطح | کینری (اسٹیج اے) | ریمپ (اسٹیج بی) | ڈیفالٹ (اسٹیج سی) |
| --------- | ------------------- | ------------------ | ------------------- |
| `sorafs_cli` بازیافت | `--anonymity-policy stage-a` یا ٹرسٹ مرحلہ | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| آرکسٹریٹر کنفیگ JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| مورچا کلائنٹ کنفیگ (`iroha.toml`) | `rollout_phase = "canary"` (پہلے سے طے شدہ) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` دستخط شدہ کمانڈز | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جاوا/اینڈروئیڈ `GatewayFetchOptions` | `setRolloutPhase("canary")` ، اختیاری `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")` ، اختیاری `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")` ، اختیاری `.ANON_STRICT_PQ` |
| جاوا اسکرپٹ آرکیسٹریٹر مددگار | `rolloutPhase: "canary"` یا `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| ازگر `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سوئفٹ `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

آرکسٹریٹر (`crates/sorafs_orchestrator/src/lib.rs:365`) کے ذریعہ استعمال ہونے والے ایک ہی اسٹیج پارسر پر تمام SDK ٹوگل میپ کا نقشہ ، لہذا کثیر زبان کی تعیناتیوں کو اسٹیج کی تشکیل کے ساتھ لاک مرحلے میں رکھا جاتا ہے۔

## کینری شیڈولنگ چیک لسٹ

1. ** پریف لائٹ (ٹی مائنس 2 ہفتوں) **

- اس بات کی تصدیق کریں کہ پچھلے دو ہفتوں میں اسٹیج براؤن آؤٹ کی شرح  = 70 ٪ فی خطہ ہے (`sorafs_orchestrator_pq_candidate_ratio`)۔
   - پروگرام گورننس ریویو سلاٹ جو کینری ونڈو کو منظور کرتا ہے۔
   - اسٹیجنگ میں `sorafs.gateway.rollout_phase = "ramp"` کو اپ ڈیٹ کریں (آرکسٹریٹر JSON اور REDEPLEAT میں ترمیم کریں) اور پروموشن پائپ لائن کو خشک کریں۔

2. ** ریلے کینری (ٹی دن) **

   - آرکسٹریٹر میں `rollout_phase = "ramp"` ترتیب دے کر اور شریک ریلے ظاہر ہونے کے ذریعہ ایک وقت میں ایک خطے کو فروغ دیں۔
   - پی کیو رچٹ ڈیش بورڈ (جس میں اب رول آؤٹ پینل شامل ہے) میں "پالیسی واقعات" اور "براؤن آؤٹ ریٹ" کی نگرانی کریں۔
   - آڈٹ اسٹوریج کے لئے عمل درآمد سے پہلے اور اس کے بعد `sorafs_cli guard-directory fetch` کے اسنیپ شاٹس کاٹ دیں۔

3. ** کلائنٹ/ایس ڈی کے کینری (ٹی پلس 1 ہفتہ) **

   - کلائنٹ کی تشکیل میں `rollout_phase = "ramp"` میں تبدیل کریں یا نامزد SDK COHORTS کے لئے `stage-b` اوور رائڈس کو پاس کریں۔
   - ٹیلی میٹری میں فرق (`sorafs_orchestrator_policy_events_total` `client_id` اور `region` کے ذریعہ گروپ کردہ) پر قبضہ کریں اور انہیں رول آؤٹ واقعہ لاگ سے منسلک کریں۔

4. ** پہلے سے طے شدہ تشہیر (T پلس 3 ہفتوں) **- ایک بار گورننس پر دستخط ہونے کے بعد ، آرکسٹریٹر اور کلائنٹ دونوں کو `rollout_phase = "default"` میں تبدیل کریں اور دستخط شدہ تیاری چیک لسٹ کو ریلیز نمونے میں گھمائیں۔

## گورننس اور شواہد چیک لسٹ

| مرحلے میں تبدیلی | پروموشن گیٹ | ثبوت بنڈل | ڈیش بورڈز اور الرٹس |
| -------------- | ------------------ | ------------------- | --------------------- |
| کینری -> ریمپ * (اسٹیج بی پیش نظارہ) * | پچھلے 14 دنوں میں براؤن آؤٹ ریٹ  = 0.7 فی پروموٹڈ خطے میں ، ارگون 2 ٹکٹ P95  ڈیفالٹ * (اسٹیج سی انفورسمنٹ) * | 30 دن مکمل ہونے والے SN16 ٹیلی میٹری برن ان ، `sn16_handshake_downgrade_total` فلیٹ پر بیس لائن ، `sorafs_orchestrator_brownouts_total` کلائنٹ کینری کے دوران صفر پر ، اور ٹوگل پراکسی رجسٹرڈ کی ریہرسل۔ | `sorafs_cli proxy set-mode --mode gateway|direct` کی نقل ، `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` کی آؤٹ پٹ ، `sorafs_cli guard-directory verify` کا لاگ ، اور دستخط شدہ بنڈل `cargo xtask soranet-rollout-capture --label default`۔ | اسی پی کیو رچٹ بورڈ کے علاوہ SN16 ڈاون گریڈ پینل `docs/source/sorafs_orchestrator_rollout.md` اور `dashboards/grafana/soranet_privacy_metrics.json` میں دستاویزی ہیں۔ |
| ایمرجنسی ڈیموشن / رول بیک تیاری | جب ڈاون گریڈ کاؤنٹرز میں اضافہ ہوتا ہے تو متحرک ، گارڈ ڈائرکٹری کی توثیق میں ناکام ہوجاتا ہے ، یا بفر `/policy/proxy-toggle` ریکارڈ برقرار رہتا ہے۔ | `docs/source/ops/soranet_transport_rollback.md` کی چیک لسٹ ، `sorafs_cli guard-directory import` / `guard-cache prune` ، `cargo xtask soranet-rollout-capture --label rollback` ، واقعہ کے ٹکٹ اور نوٹیفیکیشن ٹیمپلیٹس کی لاگ ان۔ | `dashboards/grafana/soranet_pq_ratchet.json` ، `dashboards/grafana/soranet_privacy_metrics.json` اور دونوں الرٹ پیک (`dashboards/alerts/soranet_handshake_rules.yml` ، `dashboards/alerts/soranet_privacy_rules.yml`)۔ |

- `artifacts/soranet_pq_rollout/<timestamp>_<label>/` کے تحت پیدا شدہ `rollout_capture.json` کے ساتھ ہر نمونے کو محفوظ کریں تاکہ گورننس پیکجوں میں اسکور بورڈ ، پرومٹول کے نشانات اور ہضم ہوں۔
- SHA256 کو اپلوڈڈ شواہد (پی ڈی ایف منٹ ، کیپچر بنڈل ، اسنیپ شاٹس کو محفوظ کریں) کو فروغ دینے کے منٹوں میں جوڑتا ہے تاکہ پارلیمنٹ کی منظوری کو اسٹیجنگ کلسٹر تک رسائی کے بغیر دوبارہ تیار کیا جاسکے۔
- پرومو ٹکٹ میں ٹیلی میٹری کے منصوبے کا حوالہ دیں تاکہ یہ ثابت کیا جاسکے کہ `docs/source/soranet/snnet16_telemetry_plan.md` اب بھی ڈاون گریڈ الفاظ اور الرٹ دہلیز کا ایک اہم ذریعہ ہے۔

## ڈیش بورڈ اور ٹیلی میٹری کی تازہ کاری

`dashboards/grafana/soranet_pq_ratchet.json` میں اب ایک "رول آؤٹ پلان" تشریح پینل شامل ہے جو اس پلے بک سے لنک کرتا ہے اور گورننس کے جائزوں کے لئے موجودہ مرحلے کو دکھاتا ہے تاکہ اس بات کی تصدیق کی جاسکے کہ کون سا مرحلہ فعال ہے۔ پینل کی تفصیل کو کنفیگریشن نوبس میں مستقبل میں ہونے والی تبدیلیوں کے ساتھ ہم آہنگی میں رکھیں۔

انتباہ کرنے کے لئے ، یقینی بنائیں کہ موجودہ قواعد `stage` ٹیگ کا استعمال کریں تاکہ کینری اور پہلے سے طے شدہ مراحل کو الگ الگ پالیسی دہلیز (`dashboards/alerts/soranet_handshake_rules.yml`) کو متحرک کریں۔

## رول بیک ہکس

### ڈیفالٹ -> ریمپ (اسٹیج سی -> اسٹیج بی)1. پورے بیڑے میں اسٹیج بی کو واپس لانے کے لئے `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (اور SDK کنفیگس میں اسی مرحلے کی آئینہ دار) کے ساتھ نچلے آرکیسٹریٹر۔
2. کلائنٹ کو `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` کے ذریعے محفوظ ٹرانسپورٹ پروفائل پر مجبور کریں ، ٹرانسکرپٹ پر قبضہ کریں تاکہ `/policy/proxy-toggle` ورک فلو قابل آڈٹ رہے۔
3. ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ``` کو آرکائیو گارڈ ڈائرکٹری ڈفنس ، پرومٹول آؤٹ پٹ اور ڈیش بورڈ اسکرین شاٹس کو `artifacts/soranet_pq_rollout/` کے تحت چلائیں۔

### ریمپ -> کینری (اسٹیج بی -> اسٹیج اے)

1. `sorafs_cli guard-directory import --guard-directory guards.json` کے ساتھ پروموشن سے پہلے پکڑے گئے گارڈ ڈائرکٹری اسنیپ شاٹ کو درآمد کریں اور `sorafs_cli guard-directory verify` کو دوبارہ چلائیں تاکہ ڈیمو پیکیج میں ہیش شامل ہو۔
2. سیٹ `rollout_phase = "canary"` (یا `anonymity_policy stage-a` کے ساتھ اوور رائڈ) آرکیسٹریٹر اور کلائنٹ کی تشکیل میں سیٹ کریں ، اور پھر پی کیو رچٹ ڈرل کو [PQ Ratchet Runbook] (./pq-ratchet-runbook.md) سے دہرائیں تاکہ نیچے کی پائپ لائن کو جانچنے کے لئے۔
3. گورننس کو مطلع کرنے سے پہلے پی کیو رچیٹ اور SN16 ٹیلی میٹری کے علاوہ انتباہ کے نتائج کے تازہ ترین اسکرین شاٹس منسلک کریں۔

### گارڈریل یاد دہانیاں

- حوالہ `docs/source/ops/soranet_transport_rollback.md` ہر بار جب کسی مسمار ہوتا ہے اور بعد کے کام کے لئے رول آؤٹ ٹریکر میں کسی آئٹم `TODO:` کے طور پر کسی بھی عارضی تخفیف کو ریکارڈ کرتا ہے۔
- `dashboards/alerts/soranet_handshake_rules.yml` اور `dashboards/alerts/soranet_privacy_rules.yml` کو رول بیک سے پہلے اور اس کے بعد `promtool test rules` کی کوریج کے تحت رکھیں تاکہ گرفتاری کے بنڈل کے ساتھ انتباہات کے بڑھنے کے ساتھ ساتھ دستاویزی دستاویزات بھی کی جائیں۔