---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-Rollout-plan
عنوان: SNNET-16G پوسٹ کوانٹم رول آؤٹ پلے بک
سائڈبار_لیبل: پی کیو رول آؤٹ پلان
تفصیل: سورانیٹ کے X25519+ML-KEM ہائبرڈ ہینڈ شیک کو کینری سے ڈیفالٹ تک ریلے ، کلائنٹ اور ایس ڈی کے میں فروغ دینے کے لئے آپریشنل گائیڈ۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/soranet/pq_rollout_plan.md` کا آئینہ دار ہے۔ دونوں کاپیاں ہم آہنگ رکھیں۔
:::

Snnet-16G سورنیٹ ٹرانسپورٹ کے بعد کے کوانٹم رول آؤٹ کو مکمل کرتا ہے۔ `rollout_phase` KNOBS آپریٹرز کو موجودہ مرحلے سے ایک تعصب پروموشن کو مربوط کرنے کی اجازت دیتا ہے۔

اس پلے بک کا احاطہ کرتا ہے:

- مرحلے کی تعریفیں اور نئی ترتیب knobs (`sorafs.gateway.rollout_phase` ، `sorafs.rollout_phase`) کوڈ بیس (`dashboards/grafana/soranet_privacy_metrics.json` ، `crates/iroha/src/config/user.rs:251`) میں وائرڈ۔
- ایس ڈی کے اور سی ایل آئی کے جھنڈوں کی نقشہ سازی تاکہ ہر کلائنٹ رول آؤٹ کی پیروی کرے۔
- کینری ریلے/کلائنٹ کی شیڈولنگ کی توقعات اور گورننس ڈیش بورڈز جو فروغ (`dashboards/grafana/soranet_pq_ratchet.json`) کو گیٹ دیتے ہیں۔
- رول بیک ہکس اور فائر ڈرل رن بک ([پی کیو رچیٹ رن بک] (./pq-ratchet-runbook.md)) کے حوالہ جات۔

## فیز کا نقشہ

| `rollout_phase` | موثر گمنامی انٹرنشپ | معیاری اثر | عام استعمال |
| ------------------- | -------------------------------------------------------- | --------------- |
| `canary` | `anon-guard-pq` (اسٹیج A) | کم از کم ایک پی کیو گارڈ فی سرکٹ کی ضرورت ہوتی ہے جبکہ بیڑے گرم ہوجاتے ہیں۔ | بیس لائن اور ابتدائی کینری ہفتوں۔ |
| `ramp` | `anon-majority-pq` (اسٹیج B) | تعصب> = دو تہائی کوریج کے ساتھ پی کیو ریلے کی طرف انتخاب ؛ کلاسیکی ریلے کو فال بیک کے طور پر استعمال کیا جاتا ہے۔ | ریلے خطے کے ذریعہ کینری ؛ پیش نظارہ SDK میں ٹوگل۔ |
| `default` | `anon-strict-pq` (اسٹیج سی) | صرف پی کیو صرف سرکٹس اور ہارڈن ڈاون گریڈ الارمز کو مجبور کریں۔ | ٹیلی میٹری اور گورننس سائن آف کے بعد حتمی ترقی۔ |

اگر کوئی سطح `anonymity_policy` کو بھی واضح طور پر سیٹ کرتی ہے تو ، وہ اس جزو کے مرحلے کو اوور رائٹ کرتی ہے۔ واضح مرحلے کو چھوڑنے سے اب `rollout_phase` کی قیمت کو مسترد کردیا جاتا ہے تاکہ آپریٹرز ہر ماحول میں ایک بار اسٹیج کو پلٹائیں اور مؤکلوں کو وارث ہونے دیں۔

## ترتیب حوالہ

### آرکسٹیٹر (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

آرکیسٹریٹر لوڈر رن ٹائم (`crates/sorafs_orchestrator/src/lib.rs:2229`) پر فال بیک مرحلے کو حل کرتا ہے اور اسے `sorafs_orchestrator_policy_events_total` اور `sorafs_orchestrator_pq_ratio_*` کے ذریعے بے نقاب کرتا ہے۔ تیار شدہ ٹکڑوں کے لئے تیار کردہ ٹکڑوں کے لئے `docs/examples/sorafs_rollout_stage_b.toml` اور `docs/examples/sorafs_rollout_stage_c.toml` دیکھیں۔

### مورچا کلائنٹ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` اب پارسڈ مرحلے (`crates/iroha/src/client.rs:2315`) کو ریکارڈ کرتا ہے تاکہ مددگار کمانڈز (جیسے `iroha_cli app sorafs fetch`) موجودہ مرحلے کی پہلے سے طے شدہ گمنام پالیسی کے ساتھ رپورٹ کریں۔

## آٹومیشن

دو `cargo xtask` مددگار شیڈول جنریشن اور نمونے کی گرفتاری کو خودکار کرتے ہیں۔

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
   ```

   دورانیے لاحقہ کو قبول کریں `s` ، `m` ، `h` ، یا `d`۔ کمانڈ `artifacts/soranet_pq_rollout_plan.json` اور مارک ڈاون سمری (`artifacts/soranet_pq_rollout_plan.md`) کو آؤٹ پٹ کرتا ہے جو تبدیلی کی درخواست کے ساتھ بھیجا جاسکتا ہے۔2. ** دستخطوں کے ساتھ ڈرل نمونے کی گرفتاری **

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

   کمانڈ فراہم کردہ فائلوں کو `artifacts/soranet_pq_rollout/<timestamp>_<label>/` پر کاپی کرتی ہے ، ہر نمونے کے لئے بلیک 3 ڈائجسٹوں کا حساب لگاتی ہے ، اور `rollout_capture.json` لکھتی ہے جس میں میٹا ڈیٹا اور ED25519 کے دستخط پے لوڈ سے زیادہ ہوتے ہیں۔ وہی نجی کلید استعمال کریں جو فائر ڈرل منٹ پر دستخط کرتی ہے تاکہ گورننس کیپچر کو جلدی سے توثیق کرے۔

## ایس ڈی کے اور سی ایل آئی جھنڈوں کا میٹرکس

| سطح | کینری (اسٹیج اے) | ریمپ (اسٹیج بی) | ڈیفالٹ (اسٹیج سی) |
| --------- | -------------------- | ------------------ | ------------------- |
| `sorafs_cli` بازیافت | `--anonymity-policy stage-a` یا ٹرسٹ مرحلہ | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| آرکسٹریٹر کنفیگ JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| مورچا کلائنٹ کنفیگ (`iroha.toml`) | `rollout_phase = "canary"` (پہلے سے طے شدہ) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` دستخط شدہ کمانڈز | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جاوا/اینڈروئیڈ `GatewayFetchOptions` | `setRolloutPhase("canary")` ، اختیاری `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")` ، اختیاری `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")` ، اختیاری `.ANON_STRICT_PQ` |
| جاوا اسکرپٹ آرکیسٹریٹر مددگار | `rolloutPhase: "canary"` یا `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| ازگر `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سوئفٹ `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

آرکیسٹریٹر (`crates/sorafs_orchestrator/src/lib.rs:365`) کے ذریعہ استعمال ہونے والے ایک ہی اسٹیج پارسر کا نقشہ تمام SDK کا نقشہ ، لہذا کثیر زبان کی تعیناتی تشکیل شدہ مرحلے کے ساتھ بند قدم ہے۔

## کینری شیڈولنگ چیک لسٹ

1. ** پریف لائٹ (ٹی مائنس 2 ہفتوں) **

- اس بات کی تصدیق کریں کہ پچھلے دو ہفتوں میں اسٹیج براؤن آؤٹ کی شرح  = 70 ٪ فی خطہ ہے (`sorafs_orchestrator_pq_candidate_ratio`)۔
   - گورننس ریویو سلاٹ کا شیڈول جو کینری ونڈو کو منظور کرتا ہے۔
   - اسٹیجنگ میں `sorafs.gateway.rollout_phase = "ramp"` کو اپ ڈیٹ کریں (JSON آرکسٹریٹر اور ریڈیپلائ کو ترمیم کریں) اور پروموشن پائپ لائن کو خشک کریں۔

2. ** ریلے کینری (ٹی دن) **

   - آرکسٹریٹر میں `rollout_phase = "ramp"` ترتیب دے کر اور شریک ریلے ظاہر ہونے کے ذریعہ ایک وقت میں ایک خطے کو فروغ دیں۔
   - پی کیو رچٹ ڈیش بورڈ (اب رول آؤٹ پینل کے ساتھ) گارڈ کیشے کے دو بار ٹی ٹی ایل کے لئے "پالیسی واقعات" اور "براؤن آؤٹ ریٹ" کی نگرانی کریں۔
   - آڈٹ اسٹوریج سے پہلے اور بعد میں `sorafs_cli guard-directory fetch` کے اسنیپ شاٹس لیں۔

3. ** کلائنٹ/ایس ڈی کے کینری (ٹی پلس 1 ہفتہ) **

   - کلائنٹ کی تشکیل میں `rollout_phase = "ramp"` پر سوئچ کریں یا `stage-b` اوور رائڈس کو نامزد SDK COHORTS پر پاس کریں۔
   - ٹیلی میٹری میں فرق (`sorafs_orchestrator_policy_events_total` `client_id` اور `region` کے ذریعہ گروپ کردہ) پر قبضہ کریں اور واقعہ لاگ رول آؤٹ سے منسلک ہوں۔

4. ** پہلے سے طے شدہ تشہیر (T پلس 3 ہفتوں) **

   - گورننس سائن آف کے بعد ، آرکسٹریٹر اور کلائنٹ کو `rollout_phase = "default"` میں تشکیل دیں اور ریلیز کے نمونے کے لئے دستخط شدہ تیاری چیک لسٹ کو گھمائیں۔

## گورننس اور ثبوت چیک لسٹ| مرحلے میں تبدیلی | پروموشن گیٹ | ثبوت بنڈل | ڈیش بورڈز اور الرٹس |
| -------------- | ------------------ | ------------------- | --------------------- |
| کینری -> ریمپ * (اسٹیج بی پیش نظارہ) * | اسٹیج-ایک براؤن آؤٹ ریٹ  = 0.7 فی خطے کو فروغ دیا گیا ، ارگون 2 ٹکٹ P95  ڈیفالٹ * (اسٹیج سی انفورسمنٹ) * | 30 دن کا SN16 ٹیلی میٹری برن ان مکمل ، `sn16_handshake_downgrade_total` فلیٹ پر بیس لائن ، `sorafs_orchestrator_brownouts_total` صفر کلائنٹ کینری کے دوران ، اور پراکسی ٹوگل ریہرسل لاگ ان۔ | ٹرانسکرپٹ `sorafs_cli proxy set-mode --mode gateway|direct` ، آؤٹ پٹ `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` ، لاگ `sorafs_cli guard-directory verify` ، اور دستخط شدہ بنڈل `cargo xtask soranet-rollout-capture --label default`۔ | وہی پی کیو رچٹ بورڈ اور SN16 ڈاو گریڈ پینلز `docs/source/sorafs_orchestrator_rollout.md` اور `dashboards/grafana/soranet_privacy_metrics.json` میں دستاویزی ہیں۔ |
| ایمرجنسی ڈیموشن / رول بیک تیاری | جب ڈاون گریڈ کاؤنٹرز میں اضافہ ہوتا ہے تو ، گارڈ ڈائریکٹری چیک میں ناکام ہوجاتا ہے ، یا `/policy/proxy-toggle` بفر ریکارڈز کو ڈاون گریڈ واقعات کو برقرار رکھا جاتا ہے۔ | `docs/source/ops/soranet_transport_rollback.md` چیک لسٹ ، `sorafs_cli guard-directory import` / `guard-cache prune` ، `cargo xtask soranet-rollout-capture --label rollback` لاگز ، واقعہ کے ٹکٹ اور نوٹیفکیشن ٹیمپلیٹس۔ | `dashboards/grafana/soranet_pq_ratchet.json` ، `dashboards/grafana/soranet_privacy_metrics.json` ، اور دونوں الرٹ پیک (`dashboards/alerts/soranet_handshake_rules.yml` ، `dashboards/alerts/soranet_privacy_rules.yml`)۔ |

- ہر نمونے کو `artifacts/soranet_pq_rollout/<timestamp>_<label>/` میں `rollout_capture.json` کے ساتھ اسٹور کریں تاکہ گورننس پیکٹوں میں اسکور بورڈ ، پرومٹول کے نشانات اور ہضم ہوں۔
- اپلوڈڈ شواہد (منٹ پی ڈی ایف ، کیپچر بنڈل ، اسنیپ شاٹس کو محفوظ کریں) کے شاہ 256 ہضموں کو فروغ دینے کے منٹوں میں منسلک کریں تاکہ پارلیمنٹ کی منظوری کو اسٹیجنگ کلسٹر تک رسائی کے بغیر دوبارہ تیار کیا جاسکے۔
- پروموشن ٹکٹ میں ٹیلی میٹری کے منصوبے کا حوالہ دیں تاکہ یہ ثابت کیا جاسکے کہ `docs/source/soranet/snnet16_telemetry_plan.md` ڈاون گریڈ اور الرٹ دہلیز الفاظ کے الفاظ کے لئے کیننیکل ذریعہ ہے۔

## ڈیش بورڈ اور ٹیلی میٹری کی تازہ کاری

`dashboards/grafana/soranet_pq_ratchet.json` میں اب ایک "رول آؤٹ پلان" تشریح پینل شامل ہے جو اس پلے بک سے لنک کرتا ہے اور موجودہ مرحلے کو ظاہر کرتا ہے تاکہ گورننس کے جائزے اس بات کی تصدیق کرسکیں کہ کون سا مرحلہ فعال ہے۔ پینل کی تفصیل کو آئندہ کی تبدیلیوں کے ساتھ ہم آہنگ رکھیں۔

انتباہ کرنے کے لئے ، اس بات کو یقینی بنائیں کہ موجودہ قواعد `stage` کے لیبل کا استعمال کریں تاکہ کینری اور پہلے سے طے شدہ مراحل کو الگ الگ پالیسی دہلیز (`dashboards/alerts/soranet_handshake_rules.yml`) کو متحرک کریں۔

## رول بیک ہکس

### ڈیفالٹ -> ریمپ (اسٹیج سی -> اسٹیج بی)1۔ `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (اور SDK تشکیلات میں اسی مرحلے کی آئینہ دار) کے ساتھ آرکیسٹریٹر کو ختم کردیں تاکہ اسٹیج بی پورے بیڑے میں واپس آجائے۔
2۔ کلائنٹ کو `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` کے ذریعے محفوظ ٹرانسپورٹ پروفائل پر مجبور کریں ، ٹرانسکرپٹ پر قبضہ کریں تاکہ `/policy/proxy-toggle` ورک فلو قابل نہایت قابل آیت ہو۔
3. `cargo xtask soranet-rollout-capture --label rollback-default` کو آرکائیو گارڈ ڈائریکٹری ڈفنس ، پرومٹول آؤٹ پٹ اور ڈیش بورڈ اسکرین شاٹس کو `artifacts/soranet_pq_rollout/` پر چلائیں۔

### ریمپ -> کینری (اسٹیج بی -> اسٹیج اے)

1. `sorafs_cli guard-directory import --guard-directory guards.json` کے ساتھ پروموشن سے پہلے حاصل کردہ گارڈ ڈائریکٹری اسنیپ شاٹ کو درآمد کریں اور `sorafs_cli guard-directory verify` کو دوبارہ چلائیں تاکہ ڈیمویشن پیکٹ میں ہیش شامل ہو۔
2. آرکسٹریٹر اور کلائنٹ کی تشکیل میں `rollout_phase = "canary"` (یا `anonymity_policy stage-a` کے ساتھ اوور رائڈ) کو ایڈجسٹ کریں ، پھر ڈاون گریڈ پائپ لائن کو جانچنے کے لئے [PQ Ratchet Runbook] (./pq-ratchet-runbook.md) سے PQ Ratchet ڈرل چلائیں۔
3. گورننس کو مطلع کرنے سے پہلے پی کیو رچیٹ اور SN16 ٹیلی میٹری کے علاوہ انتباہ کے نتائج کے تازہ ترین اسکرین شاٹس منسلک کریں۔

### گارڈریل یاد دہانیاں

- حوالہ `docs/source/ops/soranet_transport_rollback.md` جب بھی ڈیمویشن ہوتا ہے اور نگرانی کے لئے رول آؤٹ ٹریکر میں کسی بھی عارضی تخفیف کو آئٹم `TODO:` کے طور پر ریکارڈ کرتا ہے۔
- `dashboards/alerts/soranet_handshake_rules.yml` اور `dashboards/alerts/soranet_privacy_rules.yml` کو `promtool test rules` کوریج کے تحت ایک رول بیک سے پہلے اور اس کے بعد رکھیں تاکہ گرفتاری کے بنڈل میں الرٹ بڑھے کو دستاویزی شکل دی جاسکے۔