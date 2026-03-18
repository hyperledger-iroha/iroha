---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/transition-notes.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-منتقلی
عنوان: منتقلی Nexus پر نوٹس
تفصیل: آئینہ `docs/source/nexus_transition_notes.md` فیز بی منتقلی کے ثبوت ، آڈٹ اور تخفیف کے نظام الاوقات کا احاطہ کرتا ہے۔
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# منتقلی Nexus پر نوٹس

یہ لاگ ** فیز بی - Nexus منتقلی کی بنیادوں ** کے باقی کام کو ٹریک کرتا ہے جب تک کہ ملٹیلین رن چیک لسٹ مکمل نہ ہوجائے۔ یہ `roadmap.md` میں سنگ میل کے اندراجات کی تکمیل کرتا ہے اور B1-B4 کے ذریعہ حوالہ دینے والے ثبوت کو ایک جگہ پر رکھتا ہے تاکہ گورننس ، ایس آر ای اور ایس ڈی کے لیڈز سچائی کا ایک ہی ذریعہ بانٹ سکیں۔

## ایریا اور کیڈینس

-روٹ ٹریس آڈٹ اور ٹیلی میٹری گارڈریلز (B1/B2) ، گورننس (B3) کے ذریعہ منظور شدہ کنفیگ ڈیلٹا کا ایک سیٹ ، اور ملٹی لین لانچ ریہرسل (B4) پر فالو اپ کا احاطہ کرتا ہے۔
- عارضی کیڈینس نوٹ کی جگہ لے لیتا ہے جو پہلے یہاں تھا۔ Q1 2026 آڈٹ کے بعد سے ، `docs/source/nexus_routed_trace_audit_report_2026q1.md` میں ایک تفصیلی رپورٹ محفوظ کی گئی ہے ، اور یہ صفحہ کام کے نظام الاوقات اور تخفیف رجسٹر کو برقرار رکھتا ہے۔
- ہر روٹ ٹریس ونڈو ، گورننس ووٹ یا لانچ ریہرسل کے بعد جدولوں کو اپ ڈیٹ کریں۔ جب نمونے منتقل کردیئے جاتے ہیں تو ، یہاں نئے راستے کی عکاسی کریں تاکہ بہاو والے دستاویزات (حیثیت ، ڈیش بورڈز ، ایس ڈی کے پورٹلز) مستحکم اینکر کا حوالہ دے سکیں۔

## ثبوت کے اسنیپ شاٹ (2026 Q1-Q2)

| بہاؤ | ثبوت | مالک (زبانیں) | حیثیت | نوٹ |
| ----------- | ---------- | ---------- | -------- | ------- |
| ** B1 - روٹ ٹریس آڈٹ ** | `docs/source/nexus_routed_trace_audit_report_2026q1.md` ، `docs/examples/nexus_audit_outcomes/` | @ٹیلی میٹری آپس ، @گورننس | مکمل (Q1 2026) | تین آڈٹ ونڈوز ریکارڈ کی گئیں۔ TLS LAG `TRACE-CONFIG-DELTA` Q2 RERUN میں بند ہے۔ |
| ** B2 - ٹیلی میٹری کا علاج اور محافظ ** | `docs/source/nexus_telemetry_remediation_plan.md` ، `docs/source/telemetry.md` ، `dashboards/alerts/nexus_audit_rules.yml` | @sre-core ، @ٹیلی میٹری-او پی ایس | مکمل | الرٹ پیک ، ڈف بوٹ پالیسی اور او ٹی ایل پی بیچ سائز (`nexus.scheduler.headroom` لاگ + ہیڈ ​​روم پینل Grafana) انسٹال ؛ کوئی کھلی چھوٹ نہیں ہے۔ |
| ** B3 - کنفیگ ڈیلٹا کی منظوری ** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ، `defaults/nexus/config.toml` ، `defaults/nexus/genesis.json` | @ریلیز-اینگ ، @گورننس | مکمل | ووٹنگ GOV-2026-03-19 ریکارڈ ؛ دستخط شدہ بنڈل نیچے ٹیلی میٹری پیک کو کھانا کھلاتا ہے۔ |
| ** B4 - ملٹی لین لانچ ریہرسل ** | `docs/source/runbooks/nexus_multilane_rehearsal.md` ، `docs/source/project_tracker/nexus_rehearsal_2026q1.md` ، `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` ، `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` ، `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @گٹھ جوڑ کور ، @sre-core | مکمل (Q2 2026) | Q2 کینری ریرون بند TLS وقفے سے تخفیف ؛ ویلیویٹر منشور + `.sha256` فکس سلاٹ رینج 912-936 ، ورک لوڈ بیج `NEXUS-REH-2026Q2` اور TLS پروفائل ہیش ریرون سے۔ |

## روٹ ٹریس آڈٹ کا سہ ماہی شیڈول| ٹریس ID | ونڈو (UTC) | نتیجہ | نوٹ |
| ---------- | -------------- | --------- | ------- |
| `TRACE-LANE-ROUTING` | 2026-02-17 09: 00-09: 45 | پاس ہوا | قطار میں داخلہ P95 <= 750 ایم ایس ہدف کے نیچے اچھی طرح سے رہا۔ کسی کارروائی کی ضرورت نہیں ہے۔ |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10: 00-10: 45 | پاس ہوا | `status.md` سے منسلک OTLP ری پلے ہیش ؛ برابری SDK ڈف بوٹ نے صفر بڑھے ہوئے تصدیق کی۔ |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12: 00-12: 30 | حل | TLS وقفہ Q2 RARUN میں بند ہے۔ `NEXUS-REH-2026Q2` کے لئے ٹیلی میٹری پیک `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (`artifacts/nexus/tls_profile_rollout_2026q2/` دیکھیں) اور صفر LAGS کے TLS پروفائل ہیش پر قبضہ کرتا ہے۔ |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09: 12-10: 14 | پاس ہوا | کام کا بوجھ بیج `NEXUS-REH-2026Q2` ؛ ٹیلی میٹری پیک + `artifacts/nexus/rehearsals/2026q1/` (سلاٹ رینج 912-936) میں `artifacts/nexus/rehearsals/2026q2/` میں ایجنڈا کے ساتھ۔ |

جب ٹیبل موجودہ سہ ماہی میں اضافہ کرتا ہے تو مستقبل کے حلقوں کو نئی قطاریں شامل کریں اور درخواست میں مکمل ریکارڈز کی منتقلی کرنی چاہئے۔ `#quarterly-routed-trace-audit-schedule` اینکر کے ذریعہ روٹ ٹریس رپورٹس یا گورننس منٹ سے اس حصے کا حوالہ دیں۔

## تخفیف اور بیکلاگ

| آئٹم | تفصیل | مالک | مقصد | حیثیت/نوٹ |
| ------------ | ------------ | ------- | -------- | ------------------ |
| `NEXUS-421` | TLS پروفائل کا مکمل پھیلاؤ جو `TRACE-CONFIG-DELTA` پر پیچھے تھا ، ثبوت کو دوبارہ حاصل کریں اور تخفیف لاگ کو بند کردیں۔ | @ریلیز-اینگ ، @sre-core | Q2 2026 روٹ ٹریس ونڈو | بند۔ ریرون نے تصدیق کی کہ کوئی وقفے نہیں تھے۔ |
| `TRACE-MULTILANE-CANARY` پریپ | شیڈول Q2 ریہرسل ، ٹیلی میٹری پیک کے ساتھ فکسچر منسلک کریں اور یقینی بنائیں کہ SDK ہارنس نے تصدیق شدہ مددگار کو دوبارہ استعمال کیا ہے۔ | @ٹیلی میٹری-او پی ایس ، ایس ڈی کے پروگرام | منصوبہ بندی کا اجلاس 2026-04-30 | مکمل - ایجنڈا میٹا ڈیٹا سلاٹ/ورک بوجھ کے ساتھ `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` میں محفوظ ہے۔ ٹریکر میں دوبارہ استعمال کا استعمال نشان لگا دیا گیا ہے۔ |
| ٹیلی میٹری پیک ڈائجسٹ گردش | ہر ریہرسل/ریلیز سے پہلے `scripts/telemetry/validate_nexus_telemetry_pack.py` چلائیں اور ٹریکر کنفیگ ڈیلٹا کے ساتھ ہضم ریکارڈ کریں۔ | @ٹیلی میٹری آپس | ہر رہائی کے امیدوار کے لئے | مکمل - `telemetry_manifest.json` + `.sha256` `artifacts/nexus/rehearsals/2026q1/` میں جاری کیا گیا (سلاٹ رینج `912-936` ، بیج `NEXUS-REH-2026Q2`) ؛ ہضم کو ٹریکر اور ثبوت انڈیکس میں کاپی کیا جاتا ہے۔ |

## کنفیگ ڈیلٹا بنڈل انضمام- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` کیننیکل سمری مختلف ہے۔ جب نیا `defaults/nexus/*.toml` یا جینیسیس میں تبدیلی آتی ہے تو ، پہلے ٹریکر کو اپ ڈیٹ کریں ، پھر یہاں اہم نکات کی عکاسی کریں۔
- دستخط شدہ کنفیگ بنڈل ریہرسل ٹیلی میٹری پیک کو پاور۔ `scripts/telemetry/validate_nexus_telemetry_pack.py` کے ذریعہ توثیق شدہ پیک کو کنفیگ ڈیلٹا ثبوتوں کے ساتھ شائع کرنا ضروری ہے تاکہ آپریٹرز B4 میں استعمال ہونے والے عین مطابق نمونے کو دوبارہ پیش کرسکیں۔
- بنڈل Iroha 2 لین کے بغیر رہیں: `nexus.enabled = false` کے ساتھ تشکیلات اب لین/ڈیٹاسپیس/روٹنگ کو مسترد کریں اگر Nexus پروفائل (`--sora`) فعال نہیں ہے ، لہذا `nexus.*` سیکشنوں کو واحد لین TEMPLETS سے ہٹا دیں۔
-گورننس ووٹ لاگ (GOV-2026-03-19) کو ٹریکر اور اس نوٹ سے منسلک رکھیں تاکہ مستقبل کے ووٹوں کو منظوری کی رسم کی تلاش کے بغیر فارمیٹ کی کاپی کرسکیں۔

## لانچ ریہرسل کے ذریعہ فالو اپ

- `docs/source/runbooks/nexus_multilane_rehearsal.md` کینری پلان ، شرکاء کے روسٹر اور رول بیک اقدامات کو ریکارڈ کرتا ہے۔ رن بک کو اپ ڈیٹ کریں جب لین ٹوپولوجی یا ٹیلی میٹری ایکسپورٹر تبدیل ہوتی ہے۔
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 9 اپریل کی ریہرسل کے لئے چیک کیے گئے تمام نمونے کی فہرست دیتا ہے اور اب Q2 پریپ نوٹ/ایجنڈا شامل ہے۔ شواہد کو نیرس رکھنے کے لئے ایک وقتی ٹریکروں کی بجائے مستقبل کی ریہرسل کو ایک وقت کے ٹریکر میں شامل کریں۔
- جب رہنمائی برآمد کنندہ کی تبدیلیوں کو بیچتے ہو تو OTLP کلکٹر کے ٹکڑوں اور Grafana برآمدات (`docs/source/telemetry.md` دیکھیں) شائع کریں۔ Q1 اپ ڈیٹ نے ہیڈ روم الرٹس کو روکنے کے لئے بیچ کے سائز کو 256 نمونوں تک بڑھا دیا۔
- CI/ٹیسٹ ملٹی لین کے ثبوت اب `integration_tests/tests/nexus/multilane_pipeline.rs` میں رہتے ہیں اور ورک فلو `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`) کے ذریعے لانچ کیے جاتے ہیں ، متروک لنک `pytests/nexus/test_multilane_pipeline.py` کی جگہ لے کر ؛ ریہرسل بنڈل کو اپ ڈیٹ کرتے وقت ہیش `defaults/nexus/config.toml` (`nexus.enabled = true` ، BLAKE2B `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) کو رکھیں۔

## رن ٹائم لین لائف سائیکل

- رن ٹائم لین لائف سائیکل کے منصوبے اب ڈیٹا اسپیس پابند ہونے اور اسقاط حمل کی توثیق کرتے ہیں جب کورا/ٹائرڈ اسٹوریج مفاہمت ناکام ہوجاتی ہے ، جس سے ڈائریکٹری میں کوئی تبدیلی نہیں ہوتی ہے۔ مددگاروں نے ریٹائرڈ لینوں کے لئے کیچڈ لین ریلے کو صاف کرنے کے ل frond ریٹائرڈ لینوں کے لئے کلیئرڈ لین ریلے کو صاف کرنے کے لئے پرانی ثبوتوں کو دوبارہ استعمال کرنے سے روک دیا ہے۔
- Nexus کنفگ/لائف سائیکل مددگار (`State::apply_lane_lifecycle` ، `Queue::apply_lane_lifecycle`) کے ذریعے بغیر دوبارہ شروع کیے لینوں کو شامل کرنے/واپس لینے کے لئے منصوبوں کا اطلاق کریں۔ ایک کامیاب منصوبے کے بعد روٹنگ ، ٹی ای یو اسنیپ شاٹس اور مینی فیسٹ رجسٹریوں کو خود بخود دوبارہ شروع کردیا جاتا ہے۔
- آپریٹر کی گائیڈ: اگر منصوبہ ناکام ہوجاتا ہے تو ، چیک کریں کہ یہاں کوئی ڈیٹا اسپیس یا اسٹوریج کی جڑیں نہیں ہیں جو تخلیق نہیں کی جاسکتی ہیں (ٹائرڈ کولڈ روٹ/کورا لین ڈائریکٹریوں)۔ بنیادی راستوں کو درست کریں اور دہرائیں۔ کامیاب منصوبے ٹیلی میٹری ڈف لین/ڈیٹا اسپیس کو دوبارہ جاری کریں گے تاکہ ڈیش بورڈز نئی ٹوپولوجی کی عکاسی کریں۔

## NPOS ٹیلی میٹری اور بیک پریسر ثبوت

ریٹرو فیز بی لانچ کی ریہرسل نے فیصلہ کن ٹیلی میٹری کی گرفتاری سے یہ ثابت کیا کہ این پی او ایس پیسمیکر اور گپ شپ پرتیں بیک پریسر میں موجود ہیں۔ `integration_tests/tests/sumeragi_npos_performance.rs` میں انضمام کا استعمال ان منظرناموں کو چلاتا ہے اور جب نئی میٹرکس ظاہر ہوتی ہے تو JSON خلاصے (`sumeragi_baseline_summary::<scenario>::...`) کو آؤٹ پٹ کرتے ہیں۔ مقامی طور پر چلائیں:```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

مزید دباؤ والی ٹوپولوجیز کو دریافت کرنے کے لئے Nexus ، `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ، یا `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` سیٹ کریں۔ پہلے سے طے شدہ اقدار B4 میں استعمال ہونے والے 1 S/`k=3` پروفائل کی عکاسی کرتی ہیں۔

| اسکرپٹ/ٹیسٹ | کوریج | کلیدی ٹیلی میٹری |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | ثبوت کے بنڈل کو سیریلائز کرنے سے پہلے EMA لیٹینسی لفافوں ، قطار کی گہرائیوں اور بے کار سینڈ گیجز کو ریکارڈ کرنے کے لئے ریہرسل بلاک ٹائم کے ساتھ 12 راؤنڈ کو روکتا ہے۔ | `sumeragi_phase_latency_ema_ms` ، `sumeragi_collectors_k` ، `sumeragi_redundant_send_r` ، `sumeragi_bg_post_queue_depth*`۔ |
| `npos_queue_backpressure_triggers_metrics` | اس بات کو یقینی بنانے کے لئے ٹرانزیکشن کی قطار کو بہا دیتا ہے کہ داخلے کے التوا کو عین مطابق چلایا جاتا ہے اور صلاحیت/سنترپتی کاؤنٹرز برآمد ہوتے ہیں۔ | `sumeragi_tx_queue_depth` ، `sumeragi_tx_queue_capacity` ، `sumeragi_tx_queue_saturated` ، `sumeragi_pacemaker_backpressure_deferrals_total` ، `sumeragi_rbc_backpressure_deferrals_total`۔ |
| `npos_pacemaker_jitter_within_band` | نمونے پیسمیکر جِٹر اور ٹائم آؤٹ دیکھیں جب تک کہ یہ +/- 125 پرمل بینڈ کی تعمیل کو ثابت نہ کرے۔ | `sumeragi_pacemaker_jitter_ms` ، `sumeragi_pacemaker_view_timeout_target_ms` ، `sumeragi_pacemaker_jitter_frac_permille`۔ |
| `npos_rbc_store_backpressure_records_metrics` | اسٹور کو بہہ جانے کے بغیر سیشن/بائٹ کاؤنٹرز کی نمو ، رول بیک اور استحکام ظاہر کرنے کے لئے نرم/ہارڈ اسٹور کی حدود میں بڑے آر بی سی پے لوڈ کو دھکیل دیتا ہے۔ | `sumeragi_rbc_store_pressure` ، `sumeragi_rbc_store_sessions` ، `sumeragi_rbc_store_bytes` ، `sumeragi_rbc_backpressure_deferrals_total`۔ |
| `npos_redundant_send_retries_update_metrics` | فورسز کو دوبارہ کام کرتا ہے تاکہ گجوں کو بے کار سینڈ تناسب اور کاؤنٹرز جمع کرنے والوں پر ہدف پیشگی ، مطلوبہ ٹیلی میٹری کی اختتام سے آخر تک رابطے کو ثابت کریں۔ | `sumeragi_collectors_targeted_current` ، `sumeragi_redundant_sends_total`۔ |
| `npos_rbc_chunk_loss_fault_reports_backlog` | حصوں کو قطعی طور پر قطرے دیتے ہیں تاکہ بیک بلاگ مانیٹر خاموشی سے پے لوڈ کو نکالنے کے بجائے خرابیاں بڑھاتے ہیں۔ | `sumeragi_rbc_backlog_sessions_pending` ، `sumeragi_rbc_backlog_chunks_total` ، `sumeragi_rbc_backlog_chunks_max`۔ |

جب بھی گورننس اس بات کا ثبوت مانگتی ہے کہ بیکپریشور الارمز ریہرسل ٹوپولوجی سے ملتے ہیں تو ، JSON لائنوں کو جوڑیں جو کنٹرول کے آؤٹ پٹس کے ساتھ ساتھ Prometheus کھرچنی کے ساتھ پکڑی جاتی ہے۔

## تازہ کاری چیک لسٹ

1. نئی روٹ ٹریس ونڈوز شامل کریں اور جب آپ بلاکس کو تبدیل کرتے ہیں تو پرانے کو منتقل کریں۔
2. ہر الرٹ مینجر فالو اپ کے بعد تخفیف کی میز کو اپ ڈیٹ کریں ، چاہے کارروائی ٹکٹ کو بند کردے۔
3۔ جب ڈیلٹاس کی تشکیل کریں تو ، ایک پل کی درخواست میں ٹریکر ، اس نوٹ ، اور ڈائجسٹس ٹیلی میٹری پیک کی فہرست کو اپ ڈیٹ کریں۔
4. یہاں نئی ​​ریہرسل/ٹیلی میٹری نمونے سے لنک کریں تاکہ آئندہ روڈ میپ اپ ڈیٹس کو مختلف دستاویزات کا حوالہ دیتے ہیں بجائے اس کے کہ وہ ایڈہاک نوٹ کو مختلف کریں۔

## ثبوت انڈیکس| فعال | مقام | نوٹ |
| ------- | ---------- | ------- |
| روٹ ٹریس آڈٹ رپورٹ (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | ثبوت کے فیز B1 کے متنازعہ ماخذ ؛ `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` کے ذریعے پورٹل میں آئینہ دار۔ |
| کنفگ ڈیلٹا ٹریکر | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | ٹریس-کنفیگ ڈیلٹا ڈفف سمری ، جائزہ لینے والوں کے ابتدائی اور ووٹنگ لاگ GOV-2026-03-19 پر مشتمل ہے۔ |
| ٹیلی میٹری ریمیڈیشن پلان | `docs/source/nexus_telemetry_remediation_plan.md` | دستاویزات الرٹ پیک ، OTLP بیچ کا سائز اور B2 سے وابستہ بجٹ کے محافظ برآمدی۔ |
| ملٹی لین ریہرسل ٹریکر | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 9 اپریل کی ریہرسل نمونے ، توثیق کرنے والے مینی فیسٹ/ڈائجسٹ ، کیو 2 نوٹس/ایجنڈا اور رول بیک ثبوت کی فہرست۔ |
| ٹیلی میٹری پیک مینی فیسٹ/ڈائجسٹ (تازہ ترین) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | فکسس سلاٹ رینج 912-936 ، بیج `NEXUS-REH-2026Q2` اور ہیش آف گورننس بنڈل نمونے۔ |
| TLS پروفائل مینی فیسٹ | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Q2 ریرون میں پکڑے گئے منظور شدہ TLS پروفائل کی ہیش ؛ روٹ ٹریس ضمیموں کا حوالہ دیں۔ |
| ٹریس-ملٹیلین-کینری ایجنڈا | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Q2 ریہرسل (ونڈو ، سلاٹ رینج ، ورک لوڈ بیج ، ایکشن مالکان) کے لئے نوٹس کی منصوبہ بندی کرنا۔ |
| ریہرسل رن بک لانچ کریں `docs/source/runbooks/nexus_multilane_rehearsal.md` | آپریشنل چیک لسٹ اسٹیجنگ -> عملدرآمد -> رول بیک ؛ اپ ڈیٹ کریں جب گلیوں یا رہنمائی برآمد کنندگان کی ٹوپولاجی تبدیل ہوتی ہے۔ |
| ٹیلی میٹری پیک کی توثیق کرنے والا | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI کا ذکر B4 ریٹرو میں کیا گیا ہے۔ جب بھی پیک تبدیل ہوتا ہے تو ٹریکر کے ساتھ ہضموں کو محفوظ کریں۔ |
| کثیر الجہتی رجعت | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | ملٹی لین کنفیگس کے لئے `nexus.enabled = true` چیک کرتا ہے ، سورہ کیٹلاگ ہیشوں کو بچاتا ہے اور آرٹ فیکٹ ڈائجسٹوں کو شائع کرنے سے پہلے `ConfigLaneRouter` کے ذریعے لین لوکل کورا/انضمام لاگ راہ (`blocks/lane_{id:03}_{slug}`) کو پیش کرتا ہے۔ |