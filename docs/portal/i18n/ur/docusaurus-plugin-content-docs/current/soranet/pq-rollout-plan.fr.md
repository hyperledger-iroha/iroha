---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-Rollout-plan
عنوان: SNNET-16G پوسٹ کوانٹم تعیناتی پلے بک
سائڈبار_لیبل: پی کیو تعیناتی منصوبہ
تفصیل: سورانیٹ کے X25519+ML-KEM ہائبرڈ ہینڈ شیک کو کینری سے ڈیفالٹ تک ریلے ، مؤکلوں اور SDKs پر فروغ دینے کے لئے آپریشنل گائیڈ۔
---

::: نوٹ کینونیکل ماخذ
:::

Snnet-16G سورنیٹ ٹرانسپورٹ کے لئے پوسٹ کوانٹم کی تعیناتی کو مکمل کرتا ہے۔ `rollout_phase` KNOBS آپریٹرز کو ہر سطح کے لئے خام JSON/ٹومل میں ترمیم کیے بغیر ضرورت کے گارڈ مرحلے A سے اکثریتی کوریج اسٹیج B اور سخت PQ کرنسی اسٹیج C سے ایک تعصب پروموشن کو مربوط کرنے کی اجازت دیتا ہے۔

اس پلے بک کا احاطہ کرتا ہے:

- مرحلے کی تعریفیں اور نئی ترتیب knobs (`sorafs.gateway.rollout_phase` ، `sorafs.rollout_phase`) کوڈ بیس میں کیبلز (`crates/iroha_config/src/parameters/actual.rs:2230` ، `crates/iroha/src/config/user.rs:251`)۔
- ایس ڈی کے اور سی ایل آئی کے جھنڈوں کی نقشہ سازی تاکہ ہر کلائنٹ رول آؤٹ کی پیروی کرسکے۔
- کینری ریلے/کلائنٹ کی شیڈولنگ توقعات کے علاوہ گورننس ڈیش بورڈز جو فروغ (`dashboards/grafana/soranet_pq_ratchet.json`) کو گیٹ دیتے ہیں۔
- رول بیک ہکس اور فائر ڈرل رن بک ([پی کیو رچیٹ رن بک] (./pq-ratchet-runbook.md)) کے حوالہ جات۔

## فیز کا نقشہ

| `rollout_phase` | موثر گمنامی کا مرحلہ | پہلے سے طے شدہ اثر | عام استعمال |
| ----------------- | ------------------------------------------------------ | --------------- |
| `canary` | `anon-guard-pq` (مرحلہ A) | کم از کم ایک پی کیو گارڈ فی سرکٹ کی ضرورت ہوتی ہے جبکہ بیڑے گرم ہوجاتے ہیں۔ | بیس لائن اور کینری کے پہلے ہفتوں۔ |
| `ramp` | `anon-majority-pq` (اسٹیج B) | > = دو تہائی کوریج کے لئے پی کیو ریلے کی طرف انتخاب کے حق میں ؛ کلاسیکی ریلے فال بیک میں رہتا ہے۔ | کینری ریلے کے ذریعہ خطے ؛ SDK پیش نظارہ ٹوگل۔ |
| `default` | `anon-strict-pq` (اسٹیج سی) | صرف پی کیو صرف سرکٹس لگائیں اور ہارڈن ڈاون گریڈ الارمز لگائیں۔ | آخری تشہیر ایک بار ٹیلی میٹری اور سائن آف گورننس مکمل ہوجاتی ہے۔ |

اگر کوئی سطح بھی ایک واضح `anonymity_policy` کی وضاحت کرتی ہے تو ، یہ اس جزو کے مرحلے کو اوور رائڈ کرتی ہے۔ واضح اقدام کو چھوڑ دینا اب `rollout_phase` کی قیمت سے پہلے سے طے شدہ ہے تاکہ آپریٹرز ایک ماحول میں ایک بار سوئچ کرسکیں اور مؤکلوں کو اس کا وارث بنائیں۔

## کنفیگ ریفرنس

### آرکسٹیٹر (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

آرکیسٹریٹر لوڈر رن ٹائم (`crates/sorafs_orchestrator/src/lib.rs:2229`) میں فال بیک مرحلہ حل کرتا ہے اور اسے `sorafs_orchestrator_policy_events_total` اور `sorafs_orchestrator_pq_ratio_*` کے ذریعے بے نقاب کرتا ہے۔ تیار شدہ ٹکڑوں کے لئے تیار کردہ ٹکڑوں کے لئے `docs/examples/sorafs_rollout_stage_b.toml` اور `docs/examples/sorafs_rollout_stage_c.toml` دیکھیں۔

### مورچا کلائنٹ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` اب پارسڈ مرحلے (`crates/iroha/src/client.rs:2315`) کو بچاتا ہے تاکہ مددگار کمانڈز (جیسے `iroha_cli app sorafs fetch`) پہلے سے طے شدہ گمنامی پالیسی کے ساتھ موجودہ مرحلے کی اطلاع دے سکے۔

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
   ```دورانیے لاحقہ کو قبول کریں `s` ، `m` ، `h` ، یا `d`۔ کمانڈ `artifacts/soranet_pq_rollout_plan.json` اور مارک ڈاون سمری (`artifacts/soranet_pq_rollout_plan.md`) کو تبدیلی کی درخواست سے منسلک کرنے کے لئے جاری کرتا ہے۔

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

   کمانڈ `artifacts/soranet_pq_rollout/<timestamp>_<label>/` میں فراہم کردہ فائلوں کی کاپی کرتا ہے ، ہر نمونے کے لئے بلیک 3 ڈائجسٹ کا حساب لگاتا ہے اور میٹا ڈیٹا کے ساتھ `rollout_capture.json` لکھتا ہے اور اس کے علاوہ پے لوڈ پر ED25519 دستخط بھی ہوتا ہے۔ وہی نجی کلید استعمال کریں جو فائر ڈرل منٹ پر دستخط کرتی ہے تاکہ گورننس تیزی سے گرفتاری کی توثیق کرسکے۔

## SDK & CLI پرچم میٹرکس

| علاقہ | کینری (اسٹیج اے) | ریمپ (اسٹیج بی) | ڈیفالٹ (اسٹیج سی) |
| --------- | ----------------- | ------------------ | -------------------- |
| `sorafs_cli` بازیافت | `--anonymity-policy stage-a` یا مرحلے پر آرام کریں | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| آرکسٹریٹر کنفیگ JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| مورچا کلائنٹ کنفیگ (`iroha.toml`) | `rollout_phase = "canary"` (پہلے سے طے شدہ) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` دستخط شدہ کمانڈز | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جاوا/Android `GatewayFetchOptions` | `setRolloutPhase("canary")` ، اختیاری `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")` ، اختیاری `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")` ، اختیاری `.ANON_STRICT_PQ` |
| جاوا اسکرپٹ آرکیسٹریٹر مددگار | `rolloutPhase: "canary"` یا `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| ازگر `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سوئفٹ `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

آرکسٹریٹر (`crates/sorafs_orchestrator/src/lib.rs:365`) کے ذریعہ استعمال ہونے والے ایک ہی اسٹیج پارسر کا نقشہ تمام SDK کا نقشہ ، لہذا کثیر زبان کی تعیناتی تشکیل شدہ مرحلے کے ساتھ مقفل قدم رہتی ہے۔

## کینری شیڈولنگ چیک لسٹ

1. ** پریف لائٹ (ٹی مائنس 2 ہفتوں) **

- اس بات کی تصدیق کریں کہ پچھلے دو ہفتوں کے مقابلے میں اسٹیج براؤن آؤٹ کی شرح  = 70 ٪ خطے (`sorafs_orchestrator_pq_candidate_ratio`) ہے۔
   - گورننس ریویو سلاٹ کا شیڈول جو کینری ونڈو کو منظور کرتا ہے۔
   - اسٹیجنگ میں `sorafs.gateway.rollout_phase = "ramp"` کو اپ ڈیٹ کریں (JSON آرکسٹریٹر میں ترمیم کریں اور دوبارہ تعی .ن کریں) اور پروموشن پائپ لائن کو خشک کریں۔

2. ** ریلے کینری (ٹی دن) **

   - ایک وقت میں ایک خطے کو آرکسٹریٹر پر `rollout_phase = "ramp"` ترتیب دے کر اور حصہ لینے والے ریلے کے منشور کو فروغ دیں۔
   - گارڈ کیشے کے دو بار ٹی ٹی ایل کے لئے پی کیو رچٹ ڈیش بورڈ (جس میں اب رول آؤٹ پینل شامل ہے) میں "پالیسی واقعات" اور "براؤن آؤٹ ریٹ" کی نگرانی کریں۔
   - آڈٹ اسٹوریج کے لئے اسنیپ شاٹس سے پہلے اور اس کے بعد `sorafs_cli guard-directory fetch` پر قبضہ کریں۔

3. ** کینری کلائنٹ/ایس ڈی کے (ٹی پلس 1 ہفتہ) **

   - کلائنٹ کی تشکیل میں `rollout_phase = "ramp"` کو ٹوگل کریں یا نامزد SDK COHORTS کے لئے `stage-b` اوور رائڈس کو پاس کریں۔
   - ٹیلی میٹری میں فرق (`sorafs_orchestrator_policy_events_total` گروپ برائے `client_id` اور `region`) پر قبضہ کریں اور انہیں رول آؤٹ واقعہ لاگ سے منسلک کریں۔4. ** پہلے سے طے شدہ تشہیر (T پلس 3 ہفتوں) **

   - ایک بار گورننس کی توثیق ہوجانے کے بعد ، سوئچ آرکسٹریٹر اور کلائنٹ `rollout_phase = "default"` پر تشکیل دیتا ہے اور ریلیز نمونے میں دستخط شدہ تیاری چیک لسٹ کو چلاتا ہے۔

## چیک لسٹ گورننس اور ثبوت

| مرحلے میں تبدیلی | پروموشن گیٹ | ثبوت بنڈل | ڈیش بورڈز اور الرٹس |
| -------------- | ------------------ | ----------------- | ---------------------------- |
| کینری -> ریمپ * (اسٹیج بی پیش نظارہ) * | پچھلے 14 دنوں میں براؤن آؤٹ ریٹ مرحلہ A  = 0.7 فی پروموٹڈ خطے ، ارگون 2 ٹکٹ کی تصدیق P95  ڈیفالٹ * (اسٹیج انفورسمنٹ) * | 30 دن کی SN16 ٹیلی میٹری برن ان تک پہنچ گئی ، `sn16_handshake_downgrade_total` فلیٹ بیس لائن پر ، `sorafs_orchestrator_brownouts_total` کلائنٹ کینری کے دوران صفر پر ، اور پراکسی ٹوگل لاگ کی ریہرسل۔ | ٹرانسکرپشن `sorafs_cli proxy set-mode --mode gateway|direct` ، آؤٹ پٹ `sorafs.rollout_phase` ، لاگ `sorafs_cli guard-directory verify` ، اور بنڈل سائن `cargo xtask soranet-rollout-capture --label default`۔ | ایک ہی پی کیو رچٹ بورڈ کے علاوہ SN16 ڈاون گریڈ بورڈ `docs/source/sorafs_orchestrator_rollout.md` اور `dashboards/grafana/soranet_privacy_metrics.json` میں دستاویزی ہیں۔ |
| ایمرجنسی ڈیموشن / رول بیک تیاری | جب ڈاون گریڈ کاؤنٹرز میں اضافہ ہوتا ہے تو متحرک ہوتا ہے ، گارڈ ڈائریکٹری کی توثیق میں ناکام ہوجاتا ہے ، یا بفر `/policy/proxy-toggle` ریکارڈز کو برقرار رکھنے والے واقعات کو برقرار رکھا جاتا ہے۔ | چیک لسٹ `docs/source/ops/soranet_transport_rollback.md` ، لاگز `sorafs_cli guard-directory import` / `guard-cache prune` ، `cargo xtask soranet-rollout-capture --label rollback` ، واقعہ کے ٹکٹ اور نوٹیفکیشن ٹیمپلیٹس۔ | `dashboards/grafana/soranet_pq_ratchet.json` ، `dashboards/grafana/soranet_privacy_metrics.json` اور دو الرٹ پیک (`dashboards/alerts/soranet_handshake_rules.yml` ، `dashboards/alerts/soranet_privacy_rules.yml`)۔ |

- `artifacts/soranet_pq_rollout/<timestamp>_<label>/` کے تحت ہر نمونے کو تیار کردہ `rollout_capture.json` کے ساتھ اسٹور کریں تاکہ گورننس پیکجوں میں اسکور بورڈ ، پرومٹول کے نشانات اور ہضم ہوں۔
- پروموشن منٹ میں بھری ہوئی ثبوتوں (پی ڈی ایف منٹ ، کیپچر بنڈل ، گارڈ اسنیپ شاٹس) کے SHA256 ڈائجسٹس کو منسلک کریں تاکہ پارلیمنٹ کی منظوری کو اسٹیجنگ کلسٹر تک رسائی کے بغیر دوبارہ چلایا جاسکے۔
- پروموشن ٹکٹ میں ٹیلی میٹری کے منصوبے کا حوالہ دیں تاکہ یہ ثابت کیا جاسکے کہ `docs/source/soranet/snnet16_telemetry_plan.md` ڈاون گریڈ الفاظ اور انتباہ دہلیز کا اہم ذریعہ ہے۔

## ڈیش بورڈز اور ٹیلی میٹری اپ ڈیٹ

`dashboards/grafana/soranet_pq_ratchet.json` میں اب ایک "رول آؤٹ پلان" تشریح پینل شامل ہے جو اس پلے بک سے لنک کرتا ہے اور موجودہ مرحلے کا خاکہ پیش کرتا ہے تاکہ گورننس کے جائزے اس بات کی تصدیق کریں کہ کون سا مرحلہ فعال ہے۔ پینل کی تفصیل کو کنفیگریشن نوبس کے مستقبل کے ارتقاء کے ساتھ ہم آہنگ رکھیں۔

انتباہ کرنے کے لئے ، اس بات کو یقینی بنائیں کہ موجودہ قواعد `stage` لیبل کا استعمال کریں تاکہ کینری اور ڈیفالٹ مراحل کو الگ الگ پالیسی دہلیز (`dashboards/alerts/soranet_handshake_rules.yml`) کو متحرک کریں۔

## رول بیک ہکس

### ڈیفالٹ -> ریمپ (اسٹیج سی -> اسٹیج بی)1. آرکسٹریٹر کو `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (اور SDK تشکیلات پر اسی مرحلے کی آئینہ دار) کے ساتھ نیچے کریں تاکہ اسٹیج بی پورے بیڑے میں دوبارہ شروع ہوجائے۔
2. کلائنٹ کو `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` پر ٹرانسپورٹ پروفائل پر مجبور کریں ، ٹرانسکرپٹ پر قبضہ کریں تاکہ `/policy/proxy-toggle` علاج معالجے کا فلو قابل سماعت رہے۔
3. ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ``` کے تحت گارڈ ڈائریکٹری ڈفنس ، پرومٹول آؤٹ پٹ اور ڈیش بورڈ اسکرین شاٹس کو محفوظ کرنے کے لئے `cargo xtask soranet-rollout-capture --label rollback-default` چلائیں۔

### ریمپ -> کینری (اسٹیج بی -> اسٹیج اے)

1. `sorafs_cli guard-directory import --guard-directory guards.json` اور RERUN `sorafs_cli guard-directory verify` کے ساتھ پروموشن سے پہلے پکڑے گئے گارڈ ڈائریکٹری اسنیپ شاٹ کو درآمد کریں تاکہ ڈیموشن پیکٹ میں ہیش شامل ہو۔
2. `rollout_phase = "canary"` (یا `anonymity_policy stage-a` کے ساتھ اوور رائڈ) آرکسٹریٹر اور کلائنٹ کی تشکیل پر سیٹ کریں ، پھر پی کیو رچٹ ڈرل کو [PQ Ratchet Runbook] (./pq-ratchet-runbook.md) سے دوبارہ چلائیں تاکہ نیچے کی پائپ لائن کو ثابت کیا جاسکے۔
3. تازہ ترین پی کیو رچیٹ اور SN16 ٹیلی میٹری اسکرین شاٹس کے ساتھ ساتھ گورننس کی اطلاع سے قبل واقعے کے لاگ ان الرٹ کے نتائج بھی منسلک کریں۔

### گارڈریل یاد دہانیاں

- حوالہ `docs/source/ops/soranet_transport_rollback.md` ہر تخفیف پر اور کسی بھی عارضی تخفیف کو آئٹم `TODO:` کے طور پر ریکارڈ کرنے کے لئے رول آؤٹ ٹریکر میں ریکارڈ کریں۔
- `dashboards/alerts/soranet_handshake_rules.yml` اور `dashboards/alerts/soranet_privacy_rules.yml` کو ایک رول بیک سے پہلے اور اس کے بعد کور `promtool test rules` کے تحت رکھیں تاکہ گرفتاری کے بنڈل کے ساتھ کسی بھی انتباہ کے بہاؤ کو دستاویزی شکل دی جاسکے۔