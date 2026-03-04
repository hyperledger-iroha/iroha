---
lang: ur
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: PQ-Rollout-plan
عنوان: SNNET-16G پوسٹ کوانٹم لانچ پلان
سائڈبار_لیبل: پی کیو لانچ پلان
تفصیل: سورانیٹ x25519+ایم ایل کے کیم ہائبرڈ ہینڈ شیک کو ریلے ، کلائنٹ اور ایس ڈی کے کے ذریعہ ڈیفالٹ میں اپ گریڈ کرنے کے لئے آپریشنل گائیڈ۔
---

::: نوٹ معیاری ماخذ
یہ صفحہ `docs/source/soranet/pq_rollout_plan.md` کی عکاسی کرتا ہے۔ پرانی دستاویزات ریٹائر ہونے تک دونوں کاپیاں ایک جیسے رکھیں۔
:::

سنیٹ -16 جی پوسٹ کونٹم ٹرانسمیشن سورانیٹ کے اجراء کو مکمل کرتا ہے۔ `rollout_phase` سوئچ آپریٹرز کو اسٹیج A میں گارڈ کی ضرورت سے اسٹیج بی میں اکثریت کی کوریج سے مربوط کرنے کی اجازت دیتے ہیں اور پھر ہر سطح کے لئے خام JSON/ٹومل میں ترمیم کیے بغیر اسٹیج سی میں سخت پی کیو موڈ۔

اس پلے بک کا احاطہ کرتا ہے:

- نئے مرحلے کی تعریفیں اور ابتداء کیز (`sorafs.gateway.rollout_phase` ، `sorafs.rollout_phase`) کوڈ بیس (`dashboards/grafana/soranet_privacy_metrics.json` ، `crates/iroha/src/config/user.rs:251`) میں منسلک ہے۔
- SDK اور CLI جھنڈوں کو سیدھ میں کرنا تاکہ ہر کلائنٹ رول آؤٹ کو ٹریک کرسکے۔
- ریلے/کلائنٹ کینریوں اور گورننس بورڈز کے لئے توقعات کا شیڈولنگ جو اپ گریڈ (`dashboards/grafana/soranet_pq_ratchet.json`) کو کنٹرول کرتے ہیں۔
- رول بیک کے لئے ہکس اور فائر ڈرل دستی ([پی کیو رچیٹ رن بک] (./pq-ratchet-runbook.md)) کے لئے حوالہ جات۔

## مراحل کا نقشہ

| `rollout_phase` | اصل شناخت کے پہلے سے طے شدہ اثر کو چھپانے کا مرحلہ | معمول کا استعمال |
| ------------------- | -------------------------------------------------------- | --------------- |
| `canary` | `anon-guard-pq` (اسٹیج A) | اس بات کو یقینی بنائیں کہ ہر سرکٹ کے لئے کم از کم ایک گارڈ پی کیو موجود ہے جبکہ سسٹم گرم ہوجاتا ہے۔ | بیس لائن اور پہلے کینری ہفتوں۔ |
| `ramp` | `anon-majority-pq` (اسٹیج B) | حاصل کرنے کے لئے پی کیو ریلے کی طرف انتخاب کا تعصب> = دو تہائی کوریج ؛ کلاسیکی ریلے ایک فال بیک بیک آپشن بنی ہوئی ہے۔ | ریلے کے لئے علاقوں کے ذریعہ کینری ؛ ٹوگلز پیش نظارہ SDK. |
| `default` | `anon-strict-pq` (اسٹیج سی) | صرف PQ سرکٹس کو مجبور کریں اور ڈاون گریڈ الارم کو سخت کریں۔ | ٹیلی میٹری کی تکمیل اور گورننس کی منظوری کے بعد آخری اپ گریڈ۔ |

اگر کوئی سطح بھی ایک واضح `anonymity_policy` کا تعین کرتی ہے تو اس جزو کے مرحلے کو اوور رائڈ کرتا ہے۔ واضح مرحلے کو حذف کرنے سے یہ `rollout_phase` ویلیو پر منحصر ہوتا ہے تاکہ آپریٹرز ہر ماحول میں ایک بار مرحلے کو تبدیل کرسکیں اور مؤکلوں کو اس کا وارث بنائیں۔

## ترتیب حوالہ

### آرکسٹیٹر (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

آرکیسٹریٹر کا لوڈر رن ٹائم فال بیک (`crates/sorafs_orchestrator/src/lib.rs:2229`) حل کرتا ہے اور اسے `sorafs_orchestrator_policy_events_total` اور `sorafs_orchestrator_pq_ratio_*` کے ذریعے دکھاتا ہے۔ ریڈی میڈ مثالوں کے لئے `docs/examples/sorafs_rollout_stage_b.toml` اور `docs/examples/sorafs_rollout_stage_c.toml` دیکھیں۔

### مورچا کلائنٹ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` اب حل شدہ مرحلے (`crates/iroha/src/client.rs:2315`) کو ریکارڈ کرتا ہے تاکہ مددگار کمانڈز (جیسے `iroha_cli app sorafs fetch`) موجودہ مرحلے کی پہلے سے طے شدہ گمنام پالیسی کے ساتھ رپورٹ کرسکیں۔

## آٹومیشن

`cargo xtask` دو ٹولز ٹیبل جنریشن اور نوادرات کی گرفتاری کو خودکار کرتے ہیں۔

1. ** علاقائی جدول پیدا کریں **

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

   ادوار `s` ، `m` ، `h` ، یا `d` کے لاحقہ کو قبول کرتا ہے۔ کمانڈ `artifacts/soranet_pq_rollout_plan.json` جاری کیا گیا ہے اور تبدیلی کی درخواست کے ساتھ مارک ڈاون سمری (`artifacts/soranet_pq_rollout_plan.md`) منسلک کیا جاسکتا ہے۔

2. ** دستخطوں کے ساتھ مشقوں کے لئے نوادرات پر قبضہ کریں **

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

   کمانڈ فراہم کردہ فائلوں کو `artifacts/soranet_pq_rollout/<timestamp>_<label>/` میں کاپی کرتی ہے ، ہر نوادرات کے لئے بلیک 3 ڈائجسٹ کی گنتی کرتی ہے ، اور `rollout_capture.json` لکھتی ہے جس میں میٹا ڈیٹا اور دستخط ED25519 پر مشتمل ہے۔ وہی نجی کلید استعمال کریں جو فائر ڈرل لاگوں پر دستخط کرتی ہے تاکہ گورننس جلد تصدیق کرسکے۔## SDK اور CLI کے لئے جھنڈے سرنی

| سطح | کینری (اسٹیج اے) | ریمپ (اسٹیج بی) | ڈیفالٹ (اسٹیج سی) |
| --------- | -------------------------------------- | ------------------ | ------------------------- |
| `sorafs_cli` بازیافت | `--anonymity-policy stage-a` یا فیز انحصار | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| آرکسٹریٹر کنفیگ JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| مورچا کلائنٹ کنفیگ (`iroha.toml`) | `rollout_phase = "canary"` (پہلے سے طے شدہ) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` دستخط شدہ کمانڈز | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جاوا/اینڈروئیڈ `GatewayFetchOptions` | `setRolloutPhase("canary")` ، اختیاری `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")` ، اختیاری `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")` ، اختیاری `.ANON_STRICT_PQ` |
| جاوا اسکرپٹ آرکیسٹریٹر مددگار | `rolloutPhase: "canary"` یا `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| ازگر `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سوئفٹ `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

ایس ڈی کے میں موجود تمام ٹوگل اسی اسٹیج پارسر سے ملتے ہیں جو آرکیسٹریٹر (`crates/sorafs_orchestrator/src/lib.rs:365`) میں استعمال ہوتا ہے ، جس میں کثیر لسانی تعیناتیوں کو تشکیل شدہ مرحلے کے مطابق رکھتے ہوئے رکھا جاتا ہے۔

## کینری کے لئے شیڈولنگ کی فہرست

1. ** پریف لائٹ (ٹی مائنس 2 ہفتوں) **

- اس بات کو یقینی بنائیں کہ پچھلے دو ہفتوں کے دوران اسٹیج اے میں براؤن آؤٹ کی شرح 1 ٪ سے کم ہے اور یہ کہ ہر علاقے کے لئے پی کیو کوریج> = 70 ٪ ہے (`sorafs_orchestrator_pq_candidate_ratio`)۔
   - کینری ونڈو کے مطابق گورننس جائزہ کے لئے شیڈول سلاٹ۔
   - اسٹیجنگ میں `sorafs.gateway.rollout_phase = "ramp"` کو اپ ڈیٹ کیا گیا (آرکسٹریٹر کے JSON اور دوبارہ کام کرنے میں ترمیم کرنا) اور اپ گریڈ کے راستے کو خشک کرنے سے۔

2. ** ریلے کینری (ٹی دن) **

   - ایک وقت میں ایک زون کو `rollout_phase = "ramp"` کو آرکسٹریٹر اور حصہ لینے والے ریلے کے ظاہر کرنے کے ذریعے اپ گریڈ کریں۔
   - پی کیو رچٹ پینل (جس میں اب ایک رول آؤٹ پینل شامل ہے) میں "پالیسی واقعات" اور "براؤن آؤٹ ریٹ" کی نگرانی کرنا۔
   - آڈٹ اسٹوریج کے لئے بوٹ لگانے سے پہلے اور اس کے بعد `sorafs_cli guard-directory fetch` کے اسنیپ شاٹس پر قبضہ کریں۔

3. ** کلائنٹ/ایس ڈی کے کینری (ٹی پلس 1 ہفتہ) **

   - کلائنٹ کی ترتیبات میں `rollout_phase = "ramp"` کو الٹا کریں یا پاس اوور رائڈس `stage-b` کو مخصوص SDK بیچوں میں۔
   - ٹیلی میٹری کے اختلافات پر قبضہ کریں (`sorafs_orchestrator_policy_events_total` `client_id` اور `region` کے ذریعہ گروپ کیا گیا ہے) اور انہیں رول آؤٹ لاگ سے منسلک کریں۔

4. ** پہلے سے طے شدہ تشہیر (T پلس 3 ہفتوں) **

   - گورننس کی منظوری کے بعد ، آرکسٹریٹر اور کلائنٹ کی ترتیبات کو `rollout_phase = "default"` میں تبدیل کریں اور نوادرات کے ورژن میں سائٹ اسٹینڈ بائی چیک لسٹ کو آن کریں۔

## حکمرانی اور ثبوت کی فہرست| مرحلے میں تبدیلی | اپ گریڈ پورٹل | ثبوت پیکیج ڈیش بورڈز اور الرٹس |
| ---------------------------- | ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------
| کینری -> ریمپ * (اسٹیج بی پیش نظارہ) * | اسٹیج اے براؤن آؤٹ کی شرح 14 دن میں 1 ٪ سے کم ہے ، `sorafs_orchestrator_pq_candidate_ratio`> = 0.7 ہر اپ گریڈ شدہ خطے کے لئے ، آرگون 2 ٹکٹ P95  ڈیفالٹ * (اسٹیج سی انفورسمنٹ) * | SN16 ٹیلی میٹری کے لئے 30 دن تک برن ان چیک کریں ، `sn16_handshake_downgrade_total` کینری کلائنٹ کے دوران بیس لائن ، ` sorafs_orchestrator_brownouts_total` صفر ہے ، اور پراکسی ٹوگل کے لئے ریہرسل کی توثیق کرتا ہے۔ | متن `sorafs_cli proxy set-mode --mode gateway|direct` ، آؤٹ پٹ `promtool test rules dashboards/alerts/soranet_handshake_rules.yml` ، `sorafs_cli guard-directory verify` کو رجسٹر کریں ، اور دستخط شدہ پیکٹ `cargo xtask soranet-rollout-capture --label default`۔ | `docs/source/sorafs_orchestrator_rollout.md` اور `dashboards/grafana/soranet_privacy_metrics.json` میں دستاویزی SN16 ڈاون گریڈ بورڈ کے ساتھ ایک ہی PQ Ratchet بورڈ۔ |
| ایمرجنسی ڈیموشن / رول بیک تیاری | جب ڈاون گریڈ کاؤنٹرز بڑھتے ہیں تو متحرک ، گارڈ ڈائریکٹری چیک فیل ہوجاتا ہے ، یا بفر `/policy/proxy-toggle` ریکارڈ مستقل ڈاون گریڈ واقعات۔ | `docs/source/ops/soranet_transport_rollback.md` کی چیک لسٹ ، `sorafs_cli guard-directory import` / `guard-cache prune` ، `cargo xtask soranet-rollout-capture --label rollback` ، واقعہ کے ٹکٹ ، اور نوٹیفیکیشن ٹیمپلیٹس کے ریکارڈ۔ | `dashboards/grafana/soranet_pq_ratchet.json` ، `dashboards/grafana/soranet_privacy_metrics.json` ، اور دونوں الرٹ پیکیجز (`dashboards/alerts/soranet_handshake_rules.yml` ، `dashboards/alerts/soranet_privacy_rules.yml`)۔ |

- ہر نوادرات کو `artifacts/soranet_pq_rollout/<timestamp>_<label>/` کے تحت اسٹور کریں جس کے نتیجے میں `rollout_capture.json` ہے تاکہ گورننس پیکجوں میں اسکور بورڈ ، پرومٹول کے نشانات اور ہضم ہوں۔
- اپلوڈڈ شواہد (منٹ پی ڈی ایف ، کیپچر بنڈل ، گارڈ اسنیپ شاٹس) کے SHA256 ڈائجسٹس کو اپ گریڈ کرنے کے ل trans ٹرانسکرپٹ کو اپ گریڈ کریں تاکہ پارلیمنٹ کی منظوریوں کو اسٹیجنگ ماحول تک رسائی کے بغیر دوبارہ شروع کیا جاسکے۔
- اپ گریڈ ٹکٹ میں ٹیلی میٹری کے منصوبے کا حوالہ دیں تاکہ یہ ثابت کیا جاسکے کہ `docs/source/soranet/snnet16_telemetry_plan.md` ڈاون گریڈ الفاظ اور انتباہ کی حدود کا معیاری ذریعہ ہے۔

## ڈیش بورڈ اور ٹیلی میٹری کی تازہ کاری

`dashboards/grafana/soranet_pq_ratchet.json` میں اب ایک "رول آؤٹ پلان" فیڈ بیک پینل شامل ہے جو اس پلے بک سے لنک کرتا ہے اور موجودہ مرحلے کو دکھاتا ہے تاکہ گورننس کے جائزے اس بات کی تصدیق کرسکیں کہ کون سا مرحلہ فعال ہے۔ بورڈ کی تفصیل کو مستقبل میں نوبس کی تبدیلیوں کے ساتھ ہم آہنگی میں رکھیں۔

انتباہات کے ل sure ، یقینی بنائیں کہ موجودہ قواعد `stage` لیبل کا استعمال کریں تاکہ کینری اور پہلے سے طے شدہ مراحل الگ الگ پالیسی کی حدود (`dashboards/alerts/soranet_handshake_rules.yml`) میں اضافہ کریں۔

رول بیک کے لئے ## ہکس

### ڈیفالٹ -> ریمپ (اسٹیج سی -> اسٹیج بی)

1. بیڑے کی سطح پر اسٹیج بی پر واپس آنے کے لئے `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (اور SDK کی ترتیبات کے ذریعہ اسی مرحلے کو ہم آہنگ) کے ذریعے آرکسٹریٹر کو کم کریں۔
2. کلائنٹ کو متن کی گرفتاری کے ساتھ `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` کے ذریعے فائل کی منتقلی کو محفوظ بنانے پر مجبور کریں تاکہ `/policy/proxy-toggle` پروسیسنگ ورک فلو قابل آڈٹ رہے۔
3. ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ``` کو آرکائیو گارڈ ڈائریکٹری ڈفنس ، پرومٹول آؤٹ پٹس ، اور ڈیش بورڈز سنیپ شاٹس کو `artifacts/soranet_pq_rollout/` کے تحت چلائیں۔

### ریمپ -> کینری (اسٹیج بی -> اسٹیج اے)1. کم پیکٹ ہیشوں کو شامل کرنے کے لئے `sorafs_cli guard-directory import --guard-directory guards.json` اور Reboot `sorafs_cli guard-directory verify` کے ذریعے اپ گریڈ سے پہلے لی گئی گارڈ ڈائریکٹری اسنیپ شاٹ کو درآمد کریں۔
2. `rollout_phase = "canary"` (یا `anonymity_policy stage-a` کے ساتھ اوور رائڈ) کو آرکیسٹریٹر اور کلائنٹ کی تشکیل پر سیٹ کریں ، پھر ڈاون گریڈ راہ کو ثابت کرنے کے لئے [پی کیو رچٹ رن بک] (./pq-ratchet-runbook.md) سے پی کیو رچٹ ڈرل کو دوبارہ شروع کریں۔
3. گورننس کو مطلع کرنے سے پہلے واقعے کے لاگ میں الرٹ کے نتائج کے ساتھ اپ ڈیٹ شدہ پی کیو رچٹ اور ٹیلی میٹری SN16 اسنیپ شاٹس منسلک کریں۔

### گارڈریل یاد دہانیاں

- `docs/source/ops/soranet_transport_rollback.md` کا حوالہ دیں جب کوئی ڈاؤن ٹائم ہوتا ہے اور جاری رکھنے کے لئے رول آؤٹ ٹریکر میں کسی بھی عارضی تخفیف کو آئٹم `TODO:` کے طور پر ریکارڈ کریں۔
- `dashboards/alerts/soranet_handshake_rules.yml` اور `dashboards/alerts/soranet_privacy_rules.yml` کو کیپچر پیکیج کے ساتھ انتباہات میں دستاویز کرنے کے لئے کسی بھی رول بیک سے پہلے اور اس کے بعد `promtool test rules` کے سرورق کے تحت رکھیں۔