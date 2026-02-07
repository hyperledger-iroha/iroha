---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-Rollout-plan
عنوان: پوسٹ کوانٹم رول آؤٹ سنیٹ 16 جی کی پلے بک
سائڈبار_لیبل: پی کیو رول آؤٹ پلان
تفصیل: ہائبرڈ X25519+ML-KEM ہینڈ شیک سورنیٹ کو کینری سے ڈیفالٹ تک ریلے ، کلائنٹ اور ایس ڈی کے پر فروغ دینے کے لئے آپریشنل گائیڈ۔
---

::: نوٹ کینونیکل ماخذ
:::

Snnet-16G سورنیٹ ٹرانسپورٹ کے لئے پوسٹ کوانٹم رول آؤٹ مکمل کرتا ہے۔ نوبس `rollout_phase` آپریٹرز کو موجودہ مرحلے سے ڈٹرمینسٹک فروغ کو مربوط کرنے کی اجازت دیتا ہے ایک گارڈ کی ضرورت ہر سطح کے لئے خام JSON/ٹومل میں ترمیم کیے بغیر می اکثریت کی کوریج اور اسٹیج سی سخت پی کیو کرنسی کو اسٹیج کرنے کی ضرورت ہے۔

اس پلے بک کا احاطہ کرتا ہے:

- کوڈ بیس (`crates/iroha_config/src/parameters/actual.rs:2230` ، `crates/iroha/src/config/user.rs:251`) میں مرحلے کی تعریفیں اور نئی تشکیل KNOBS (`sorafs.gateway.rollout_phase` ، `sorafs.rollout_phase`)۔
- ایس ڈی کے اور سی ایل آئی کے جھنڈوں کی نقشہ سازی تاکہ ہر کلائنٹ رول آؤٹ کو ٹریک کرسکے۔
- ریلے/کلائنٹ اور گورننس ڈیش بورڈز کے لئے کینری شیڈولنگ کی توقعات ، جو گیٹ پروموشن (`dashboards/grafana/soranet_pq_ratchet.json`)۔
- رول بیک ہکس اور لنکس فائر ڈرل رن بک ([پی کیو رچیٹ رن بک] (./pq-ratchet-runbook.md)) سے لنک۔

## فیز کا نقشہ

| `rollout_phase` | موثر گمنامی کا مرحلہ | پہلے سے طے شدہ اثر | عام استعمال |
| ------------------ | ----------- | ---------------- | ---------------- |
| `canary` | `anon-guard-pq` (مرحلہ A) | کم از کم ایک پی کیو گارڈ فی سرکٹ کی ضرورت ہوتی ہے جبکہ بیڑے گرم ہو رہے ہیں۔ | بیس لائن اور کینری کے ابتدائی ہفتوں۔ |
| `ramp` | `anon-majority-pq` (اسٹیج B) | > = دو تہائی کوریج کے لئے پی کیو ریلے کی طرف شفٹ سلیکشن ؛ کلاسیکی ریلے فال بیک ہی رہتا ہے۔ | علاقائی ریلے کینری ؛ SDK پیش نظارہ ٹوگل۔ |
| `default` | `anon-strict-pq` (اسٹیج سی) | صرف پی کیو صرف سرکٹس پر مجبور کریں اور ڈاون گریڈ الارم کو مضبوط بنائیں۔ | ٹیلی میٹری اور سائن آف گورننس کی تکمیل کے بعد حتمی فروغ۔ |

اگر سطح بھی ایک واضح `anonymity_policy` کی وضاحت کرتی ہے تو ، یہ اس جزو کے مرحلے کو اوور رائڈ کرتی ہے۔ ایک واضح مرحلے کی عدم موجودگی اب `rollout_phase` کی قیمت سے موخر کردی گئی ہے تاکہ آپریٹرز ہر ماحول میں ایک بار مرحلے کو تبدیل کرسکیں اور مؤکلوں کو اس کا وارث بنائیں۔

## حوالہ کنفیگریشنز

### آرکسٹیٹر (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

لوڈر آرکسٹریٹر رن ٹائم (`crates/sorafs_orchestrator/src/lib.rs:2229`) پر فال بیک مرحلے کو حل کرتا ہے اور اسے `sorafs_orchestrator_policy_events_total` اور `sorafs_orchestrator_pq_ratio_*` کے ذریعے پیش کرتا ہے۔ ریڈی میڈ ٹکڑوں کے لئے `docs/examples/sorafs_rollout_stage_b.toml` اور `docs/examples/sorafs_rollout_stage_c.toml` دیکھیں۔

### مورچا کلائنٹ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` اب پارسڈ مرحلے (`crates/iroha/src/client.rs:2315`) کو اسٹور کرتا ہے تاکہ مددگار کمانڈز (مثال کے طور پر `iroha_cli app sorafs fetch`) پہلے سے طے شدہ گمنامی کی پالیسی کے ساتھ موجودہ مرحلے کی اطلاع دے سکے۔

## آٹومیشن

دو `cargo xtask` مددگار شیڈول جنریشن کو خودکار کرتے ہیں اور نوادرات کی گرفت کرتے ہیں۔

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

   دورانیے لاحقہ کو قبول کریں `s` ، `m` ، `h` ، یا `d`۔ ٹیم `artifacts/soranet_pq_rollout_plan.json` اور مارک ڈاون سمری (`artifacts/soranet_pq_rollout_plan.md`) تیار کرتی ہے جو تبدیلی کی درخواست کے ساتھ منسلک ہوسکتی ہے۔

2.

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```کمانڈ مخصوص فائلوں کو `artifacts/soranet_pq_rollout/<timestamp>_<label>/` پر کاپی کرتی ہے ، ہر نوادرات کے لئے بلیک 3 ڈائجسٹوں کا حساب لگاتی ہے اور پے لوڈ کے دوران میٹا ڈیٹا اور ED255519 کے دستخط کے ساتھ `rollout_capture.json` لکھتی ہے۔ وہی نجی کلید استعمال کریں جس نے فائر ڈرل منٹ پر دستخط کیے تاکہ گورننس تیزی سے گرفتاری کی توثیق کرسکے۔

## SDK & CLI پرچم میٹرکس

| سطح | کینری (اسٹیج اے) | ریمپ (اسٹیج بی) | ڈیفالٹ (اسٹیج سی) |
| --------- | ------------------ | ------------------ | ---- |
| `sorafs_cli` بازیافت | `--anonymity-policy stage-a` یا فیز پر انحصار کریں `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| آرکسٹریٹر کنفیگ JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| مورچا کلائنٹ کنفیگ (`iroha.toml`) | `rollout_phase = "canary"` (پہلے سے طے شدہ) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` دستخط شدہ کمانڈز | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جاوا/Android `GatewayFetchOptions` | `setRolloutPhase("canary")` ، اختیاری `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")` ، اختیاری `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")` ، اختیاری `.ANON_STRICT_PQ` |
| جاوا اسکرپٹ آرکیسٹریٹر مددگار | `rolloutPhase: "canary"` یا `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| ازگر `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سوئفٹ `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

تمام ایس ڈی کے ٹوگل کو اسی اسٹیج پارسر میں نقشہ بنایا گیا ہے جیسے آرکسٹریٹر (`crates/sorafs_orchestrator/src/lib.rs:365`) ، لہذا کثیر زبان کی تعیناتی تشکیل شدہ مرحلے کے ساتھ لاک مرحلے میں موجود ہے۔

## کینری شیڈولنگ چیک لسٹ

1. ** پریف لائٹ (ٹی مائنس 2 ہفتوں) **

- اس بات کو یقینی بنائیں کہ پچھلے دو ہفتوں کے لئے اس مرحلے میں براؤن آؤٹ ریٹ  = 70 ٪ (`sorafs_orchestrator_pq_candidate_ratio`)۔
   - ایک گورننس ریویو سلاٹ کا شیڈول جو کینری ونڈو کی توثیق کرتا ہے۔
   - اسٹیجنگ میں `sorafs.gateway.rollout_phase = "ramp"` کو اپ ڈیٹ کریں (آرکسٹریٹر JSON اور REDEPLEAT میں ترمیم کریں) اور ڈرائی رن پروموشن پائپ لائن چلائیں۔

2. ** ریلے کینری (ٹی دن) **

   - ایک وقت میں ایک خطے کو `rollout_phase = "ramp"` کو آرکیسٹریٹر اور حصہ لینے والے ریلے کے منشور میں ترتیب دے کر فروغ دیں۔
   - ڈبل ٹی ٹی ایل گارڈ کیشے کے لئے پی کیو رچٹ ڈیش بورڈ (رول آؤٹ پینل کے ساتھ) پر "پالیسی واقعات فی نتیجہ" اور "براؤن آؤٹ ریٹ" کی نگرانی کریں۔
   - آڈٹ اسٹوریج کے لئے `sorafs_cli guard-directory fetch` کے سنیپ شاٹس سے پہلے اور بعد میں لیں۔

3. ** کلائنٹ/ایس ڈی کے کینری (ٹی پلس 1 ہفتہ) **

   - کلائنٹ کی تشکیل میں `rollout_phase = "ramp"` کو سوئچ کریں یا ہدف SDK COHORTS کے لئے `stage-b` اوور رائڈس کو پاس کریں۔
   - ٹیلی میٹری میں فرق (`sorafs_orchestrator_policy_events_total` ، `client_id` اور `region` کے ذریعہ گروپ کردہ) پر قبضہ کریں اور انہیں رول آؤٹ واقعہ لاگ سے منسلک کریں۔

4. ** پہلے سے طے شدہ تشہیر (T پلس 3 ہفتوں) **

   - سائن آف گورننس کے بعد ، آرکسٹریٹر اور کلائنٹ کو `rollout_phase = "default"` پر تشکیل دیں اور ریلیز آرٹ فیکٹ میں دستخط شدہ تیاری چیک لسٹ کو گھمائیں۔

## گورننس اور ثبوت چیک لسٹ| مرحلے میں تبدیلی | پروموشن گیٹ | ثبوت بنڈل | ڈیش بورڈز اور الرٹس |
| -------------- | ------------------ | ------------------- | --------------------- |
| کینری -> ریمپ * (اسٹیج بی پیش نظارہ) * | اسٹیج-ایک براؤن آؤٹ ریٹ  = 0.7 کے ذریعہ خطے کی تشہیر ، ارگون 2 ٹکٹ کی تصدیق P95  ڈیفالٹ * (اسٹیج سی انفورسمنٹ) * | 30 دن کی SN16 ٹیلی میٹری برن ان ، `sn16_handshake_downgrade_total` بیس لائن پر فلیٹ ، کلائنٹ کینری کے دوران `sorafs_orchestrator_brownouts_total` صفر ، اور پراکسی ٹوگل ریہرسل ریکارڈ کیا گیا۔ | ٹرانسکرپٹ `sorafs_cli proxy set-mode --mode gateway|direct` ، آؤٹ پٹ `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` ، لاگ `sorafs_cli guard-directory verify` ، دستخط شدہ بنڈل `cargo xtask soranet-rollout-capture --label default`۔ | وہی پی کیو رچٹ بورڈ پلس SN16 ڈاون گریڈ پینل `docs/source/sorafs_orchestrator_rollout.md` اور `dashboards/grafana/soranet_privacy_metrics.json` میں بیان کیا گیا ہے۔ |
| ایمرجنسی ڈیموشن / رول بیک تیاری | `/policy/proxy-toggle` بفر میں ڈاون گریڈ کاؤنٹرز کے پھٹ ، گارڈ ڈائرکٹری توثیق کی ناکامی ، یا مستقل ڈاون گریڈ واقعات کی وجہ سے متحرک۔ | `docs/source/ops/soranet_transport_rollback.md` ، لاگس `sorafs_cli guard-directory import` / `guard-cache prune` ، `cargo xtask soranet-rollout-capture --label rollback` ، واقعہ کے ٹکٹ اور نوٹیفکیشن ٹیمپلیٹس سے چیک لسٹ۔ | `dashboards/grafana/soranet_pq_ratchet.json` ، `dashboards/grafana/soranet_privacy_metrics.json` اور دونوں الرٹ پیک (`dashboards/alerts/soranet_handshake_rules.yml` ، `dashboards/alerts/soranet_privacy_rules.yml`)۔ |

- ہر نوادرات کو `artifacts/soranet_pq_rollout/<timestamp>_<label>/` میں تیار کردہ `rollout_capture.json` کے ساتھ ساتھ رکھیں تاکہ گورننس پیکٹوں میں اسکور بورڈ ، پرومٹول کے نشانات اور ہضم شامل ہوں۔
- ڈاؤن لوڈ کردہ شواہد (منٹ پی ڈی ایف ، کیپچر بنڈل ، گارڈ اسنیپ شاٹس) کے شاہ 256 ڈائجسٹس کو فروغ دینے کے منٹوں میں منسلک کریں تاکہ پارلیمنٹ کی منظوری کو اسٹیجنگ کلسٹر تک رسائی کے بغیر دوبارہ پیش کیا جاسکے۔
- پروموشن ٹکٹ میں ٹیلی میٹری کے منصوبے کا حوالہ دیں تاکہ اس بات کی تصدیق کی جاسکے کہ `docs/source/soranet/snnet16_telemetry_plan.md` ڈاون گریڈ الفاظ اور الرٹ دہلیز کے لئے کیننیکل ذریعہ بنی ہوئی ہے۔

## ڈیش بورڈ اور ٹیلی میٹری کی تازہ کاری

`dashboards/grafana/soranet_pq_ratchet.json` میں اب ایک "رول آؤٹ پلان" تشریح پینل موجود ہے جو اس پلے بک کا حوالہ دیتا ہے اور موجودہ مرحلے کو دکھاتا ہے تاکہ گورننس کے جائزے فعال مرحلے کی تصدیق کرسکیں۔ پینل کی تفصیل مستقبل کے نوبس کی تبدیلیوں کے ساتھ ہم آہنگی میں رکھیں۔

انتباہ کرنے کے لئے ، اس بات کو یقینی بنائیں کہ موجودہ قواعد لیبل `stage` کا استعمال کرتے ہیں تاکہ کینری اور پہلے سے طے شدہ مراحل ٹرگر کو الگ الگ پالیسی دہلیز (`dashboards/alerts/soranet_handshake_rules.yml`) ٹرگر کریں۔

## رول بیک ہکس

### ڈیفالٹ -> ریمپ (اسٹیج سی -> اسٹیج بی)

1. ڈیموٹ آرکیسٹریٹر `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` کے ذریعے (اور اسی مرحلے کو ایس ڈی کے کنفیگس میں آئینہ دار) تاکہ اسٹیج بی کو پورے بیڑے میں بحال کیا جاسکے۔
2. `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` کے ذریعے ایک محفوظ ٹرانسپورٹ پروفائل میں کلائنٹ کو مجبور کریں ، آڈیٹیبلٹی ورک فلو `/policy/proxy-toggle` کے لئے ایک ٹرانسکرپٹ کی بچت کریں۔
3. ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ``` کے تحت آرکائیو گارڈ ڈائریکٹری ڈفنس ، پرومٹول آؤٹ پٹ اور ڈیش بورڈ اسکرین شاٹس کے لئے `cargo xtask soranet-rollout-capture --label rollback-default` چلائیں۔

### ریمپ -> کینری (اسٹیج بی -> اسٹیج اے)1. `sorafs_cli guard-directory import --guard-directory guards.json` کے ذریعے پروموشن سے پہلے پکڑے گئے گارڈ ڈائریکٹری اسنیپ شاٹ کو درآمد کریں اور `sorafs_cli guard-directory verify` کو دوبارہ چلائیں تاکہ ڈیموشن پیکٹ میں ہیش شامل ہو۔
2. `rollout_phase = "canary"` (یا `anonymity_policy stage-a` کو اوور رائڈ `anonymity_policy stage-a`) آرکسٹریٹر اور کلائنٹ کی تشکیل میں سیٹ کریں ، پھر ڈاون گریڈ پائپ لائن کو ثابت کرنے کے لئے [PQ RATCHET RUNBOOK] (./pq-ratchet-runbook.md) سے PQ RACHETT ڈرل کو دہرائیں۔
3. اپ ڈیٹ شدہ پی کیو رچٹ اور SN16 ٹیلی میٹری اسکرین شاٹس پلس الرٹ نتائج کے نتائج گورننس کو مطلع کرنے سے پہلے واقعے کے لاگ ان کے نتائج کے نتائج۔

### گارڈریل یاد دہانیاں

- بعد کے کام کے لئے رول آؤٹ ٹریکر میں `docs/source/ops/soranet_transport_rollback.md` کو ہر تخفیف میں اور عارضی تخفیف ریکارڈ کریں۔
- `dashboards/alerts/soranet_handshake_rules.yml` اور `dashboards/alerts/soranet_privacy_rules.yml` کو `promtool test rules` کے ذریعہ رول بیک سے پہلے اور اس کے بعد رکھیں تاکہ گرفتاری کے بنڈل کے ساتھ انتباہی بڑھے ہوئے دستاویزات بھی ہوں۔