---
lang: ar
direction: rtl
source: docs/source/sumeragi_pacemaker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b0e7a0ca09fb294fed62bae221a6fd82ce07d9cf0802d90d960afaa5bcec40e9
source_last_modified: "2025-12-09T14:07:26.178015+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/sumeragi_pacemaker.md -->

# Sumeragi Pacemaker - المؤقتات، backoff، و jitter

توضح هذه المذكرة سياسة pacemaker (timer) في Sumeragi وتقدم ارشادات للمشغلين وامثلة. تتحكم المؤقتات في وقت تقديم القادة للمقترحات ومتى يقترح المدققون/يدخلون view جديدة بعد الخمول.

الحالة: تم تنفيذ نافذة اساس مشتقة من EMA، وارضية RTT، وحدود jitter/backoff قابلة للضبط. اصبح فاصل اقتراح pacemaker الان محصورا بين هدف block-time و propose timeout مع ارضية RTT، ومقيدا بـ `sumeragi.advanced.pacemaker.max_backoff_ms`. (`effective_block_time_ms` = `block_time_ms` scaled by `pacing_factor_bps`). يتم تطبيق jitter بشكل حتمي لكل عقدة ولكل (height, view).

## المفاهيم
- النافذة الاساسية: متوسط متحرك اسي لمراحل التوافق المرصودة
  (propose, collect_da, collect_prevote, collect_precommit, commit). يتم تهيئة
  EMA من `sumeragi.advanced.npos.timeouts.*_ms`; حتى تتوفر عينات كافية فهي تطابق القيم
  الافتراضية المهيأة عمليا. يتم تصدير EMA لـ `collect_aggregator` للملاحظة
  لكنها غير مدرجة في نافذة pacemaker. تظهر القيم الملساء عبر
  `sumeragi_phase_latency_ema_ms{phase=...}`.
- مضاعف backoff: `sumeragi.advanced.pacemaker.backoff_multiplier` (الافتراضي 1). كل timeout يضيف `base * multiplier` الى النافذة الحالية.
- ارضية RTT: `avg_rtt_ms * sumeragi.advanced.pacemaker.rtt_floor_multiplier` (الافتراضي 2). تمنع timeouts العدوانية على الروابط عالية الكمون.
- السقف: `sumeragi.advanced.pacemaker.max_backoff_ms` (الافتراضي 60_000 ms). حد اعلى صارم للنافذة.
- بذرة فاصل الاقتراح: `max(effective_block_time_ms, propose_timeout_ms * rtt_floor_multiplier)` ولا تتجاوز `sumeragi.advanced.pacemaker.max_backoff_ms`. (`effective_block_time_ms` = `block_time_ms` scaled by `pacing_factor_bps`). هذا هو الفاصل في الحالة المستقرة حتى دون backoff.

تحديث النافذة الفعلية عند timeout:
- `window = min(cap, max(window + base * backoff_mul, avg_rtt * rtt_floor_mul))`
- عند عدم وجود عينات RTT تكون ارضية RTT = 0.

القياسات المعروضة (انظر telemetry.md):
- Runtime: `sumeragi_pacemaker_backoff_ms`, `sumeragi_pacemaker_rtt_floor_ms`, `sumeragi_phase_latency_ema_ms{phase=...}`
- لقطة REST: `/v2/sumeragi/phases` تتضمن الان `ema_ms` الى جانب احدث
  زمنيات المراحل كي تتمكن لوحات المتابعة من رسم اتجاه EMA دون سحب
  Prometheus مباشرة.
- Config: `sumeragi.advanced.pacemaker.backoff_multiplier`, `sumeragi.advanced.pacemaker.rtt_floor_multiplier`, `sumeragi.advanced.pacemaker.max_backoff_ms`

## سياسة jitter
لتجنب اثر القطيع (timeouts المتزامنة)، يدعم pacemaker jitter صغيرا لكل عقدة حول النافذة الفعلية.

jitter حتمي لكل عقدة:
- المصدر: `blake2(chain_id || peer_id || height || view)` -> 64-bit، يتم تحجيمه الى [-J, +J].
- نطاق jitter الموصى به: +/-10% من `window` المحسوبة.
- يطبق مرة واحدة لكل نافذة (height, view)؛ لا تغيره داخل نفس view.

Pseudocode:
```
base = ema_total_ms(view, height)  // seeded by sumeragi.advanced.npos.timeouts.*_ms
window = min(cap, max(prev + base * backoff_mul, avg_rtt * rtt_floor_mul))
seed = blake2(chain_id || peer_id || height || view)
u = (seed % 10_000) as f64 / 10_000.0  // [0, 1)
jfrac = 0.10 // 10% jitter band
jitter = (u * 2.0 - 1.0) * jfrac * window.as_millis() as f64
window_jittered_ms = (window.as_millis() as f64 + jitter).clamp(0.0, cap_ms as f64)
```

ملاحظات:
- jitter حتمي لكل عقدة ولكل (height, view) ولا يتطلب عشوائية خارجية.
- اجعل النطاق صغيرا (<= 10%) للحفاظ على الاستجابة وتقليل stampede.
- jitter يؤثر على liveness لكنه لا يؤثر على safety.

Telemetria:
- `sumeragi_pacemaker_jitter_ms` — مقدار jitter المطلق (ms) المطبق.
- `sumeragi_pacemaker_jitter_frac_permille` — نطاق jitter المهيأ (permille).

## ارشادات للمشغلين

LAN/PoA منخفضة الكمون
- backoff_multiplier: 1-2
- rtt_floor_multiplier: 2
- max_backoff_ms: 5_000-10_000
- المبرر: ابقاء الاقتراحات متكررة؛ تعاف قصير عند stall.

WAN جغرافية موزعة
- backoff_multiplier: 2-3
- rtt_floor_multiplier: 3-5
- max_backoff_ms: 30_000-60_000
- المبرر: تجنب churn العدواني عبر روابط RTT عالية؛ تحمل bursts.

شبكات عالية التذبذب او متنقلة
- backoff_multiplier: 3-4
- rtt_floor_multiplier: 4-6
- max_backoff_ms: 60_000
- المبرر: تقليل تغييرات view المتزامنة؛ فكر في تفعيل jitter عندما يتحمل النشر commits ابطا.

## امثلة

1) LAN بمتوسط RTT ~= 2 ms, base = 2000 ms, cap = 10_000 ms
- backoff_mul = 1, rtt_floor_mul = 2
- اول timeout: window = max(0 + 2000, 2*2) = 2000 ms
- ثاني timeout: window = min(10_000, 2000 + 2000) = 4000 ms

2) WAN بمتوسط RTT ~= 80 ms, base = 4000 ms, cap = 60_000 ms
- backoff_mul = 2, rtt_floor_mul = 3
- اول timeout: window = max(0 + 8000, 80*3) = 8000 ms
- ثاني timeout: window = min(60_000, 8000 + 8000) = 16_000 ms

3) مع نطاق jitter 10% (للتوضيح فقط، غير منفذ)
- window = 16_000 ms, jitter in [-1_600, +1_600] ms -> `window' in [14_400, 17_600]` ms لكل عقدة

## المراقبة
- Trend backoff: `avg_over_time(sumeragi_pacemaker_backoff_ms[5m])`
- Inspect RTT floor: `avg_over_time(sumeragi_pacemaker_rtt_floor_ms[5m])`
- قارن EMA مع histogram: `sumeragi_phase_latency_ema_ms{phase=...}` الى جانب النسب المئوية المطابقة في `sumeragi_phase_latency_ms{phase=...}`
- تحقق من config: `max(sumeragi.advanced.pacemaker.backoff_multiplier)`, `max(sumeragi.advanced.pacemaker.rtt_floor_multiplier)`, `max(sumeragi.advanced.pacemaker.max_backoff_ms)`

## الحتمية والسلامة
- المؤقتات/backoff/jitter تؤثر فقط على توقيت اطلاق المقترحات/تغييرات view؛ ولا تؤثر على صحة التواقيع او قواعد commit certificate.
- اجعل اي عشوائية حتمية لكل عقدة ولكل (height, view). تجنب time-of-day او RNG للنظام في مسارات التوافق الحرجة.

</div>
