---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: ar
direction: rtl
source: docs/amx.md
status: complete
translator: manual
source_hash: 563ec4d7d4d96fa04a7b210a962b9927046985eb01d1de0954575cf817f9f226
source_last_modified: "2025-11-09T19:43:51.262828+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/amx.md (AMX Execution & Operations Guide) -->

# دليل تنفيذ وتشغيل AMX

**الحالة:** مسودة (NX‑17)  
**الجمهور:** فريق بروتوكول النواة، مهندسو AMX/الإجماع، فرق SRE/التليمترية،
فِرق SDK وTorii  
**السياق:** يستكمل عنصر الـ roadmap: “التوثيق (owner: Docs) — تحديث
`docs/amx.md` بمخططات زمنية، وفهرس للأخطاء، وتوقعات للمشغّلين، وإرشادات
للمطورين حول توليد/استخدام PVOs”.

## الملخّص

تسمح المعاملات الذرية عبر مساحات البيانات (AMX) لعملية إرسال واحدة بأن
تؤثر على عدة data spaces (DS) مع الحفاظ على نهائية slot مدتها ثانية واحدة،
وأكواد فشل حتمية، وسرّية fragments في الـ DS الخاصة. يوضّح هذا الدليل
نموذج التوقيت، وطريقة التعامل الكانونية مع الأخطاء، ومتطلبات الأدلة
للمشغّلين، وتوقعات المطورين فيما يخص Proof Verification Objects (PVOs)،
حتى يكون deliverable الـ roadmap مكتفيًا ذاتيًا خارج ورقة تصميم Nexus
(`docs/source/nexus.md`).

الضمانات الأساسية:

- كل إرسال AMX يحصل على budgets حتمية لمراحل prepare/commit؛ أي تجاوزات
  تؤدي إلى إجهاض (abort) مع أكواد موثّقة بدلًا من تجميد lanes.
- عينات الـ DA التي لا تلتزم بالـ budget تنقل المعاملة إلى الـ slot
  التالي وتصدر Telemetry واضحة (`missing-availability warning`) بدلًا من
  خفض معدل throughput بصمت.
- تفصل PVOs بين عمليات إثبات ثقيلة (proving) وبين نافذة الـ 1 s، من خلال
  السماح للعملاء/الباتشرز preregister بآرتيفاكتات يتحقق منها Nexus host
  بسرعة داخل الـ slot.

## نموذج توقيت الـ Slot

### المخطط الزمني

```text
t=0ms           70ms             300ms              600ms       840ms    1000ms
│─────────┬───────────────┬───────────────────┬──────────────┬──────────┬────────│
│         │               │                   │              │          │        │
│  Mempool│Proof build + DA│Consensus PREP/COM │ IVM/AMX exec │Settlement│ Guard  │
│  ingest │sample (≤300ms) │(≤300ms)           │(≤250ms)      │(≤40ms)   │(≤40ms) │
```

- تتماشى الـ budgets مع خطة الـ ledger العامة: mempool 70 ms، commit لبيانات
  التوافر DA ≤300 ms، الإجماع 300 ms، تنفيذ IVM/AMX خلال 250 ms، التسوية
  (settlement) خلال 40 ms، وguard خلال 40 ms.
- المعاملات التي تتجاوز نافذة DA يعاد جدولتها بشكل حتمي؛ أما باقي
  التجاوزات فتظهر كأكواد مثل `AMX_TIMEOUT` أو
  `SETTLEMENT_ROUTER_UNAVAILABLE`.
- يمتصّ جزء الـ guard عمليات تصدير التليمترية والتدقيق النهائي كي ينتهي
  الـ slot في 1 s حتى لو تأخّر الـ exporters قليلًا.

### مخطط swim lane عبر مساحات البيانات

```text
Client        DS A (public)        DS B (private)        Nexus Lane        Settlement
  │ submit tx │                     │                     │                 │
  │──────────▶│ prepare fragment    │                     │                 │
  │           │ proof + DA part     │ prepare fragment    │                 │
  │           │───────────────┬────▶│ proof + DA part     │                 │
  │           │               │     │─────────────┬──────▶│ Merge proofs    │
  │           │               │     │             │       │ verify PVO/DA   │
  │           │               │     │             │       │────────┬────────▶ apply
  │◀──────────│ result + code │◀────│ result + code │◀────│ outcome│          receipt
```

يجب أن يكمل كل fragment من DS نافذة prepare البالغة 30 ms قبل أن تقوم
الـ lane بتجميع الـ slot. تبقى الـ proofs الناقصة في mempool للـ slot
التالي بدلًا من تعطيل الـ peers.

### قائمة تحقق للـ Instrumentation

| المقياس / التتبع | المصدر | SLO / تنبيه | ملاحظات |
|------------------|--------|-------------|---------|
| `iroha_slot_duration_ms` (histogram) / `iroha_slot_duration_ms_latest` (gauge) | `iroha_telemetry` | p95 ≤ 1000 ms | Gate CI كما في `ans3.md`. |
| `iroha_da_quorum_ratio` | `iroha_telemetry` (hook على commit) | ≥0.95 لكل نافذة 30 دقيقة | مشتقة من Telemetry إعادة الجدولة للـ DA؛ كل block يحدّث الـ gauge. |
| `iroha_amx_prepare_ms` | مضيف IVM | p95 ≤ 30 ms لكل DS scope | يقود إجهاضات `AMX_TIMEOUT`. |
| `iroha_amx_commit_ms` | مضيف IVM | p95 ≤ 40 ms لكل DS scope | يغطي دمج الدلتا + تنفيذ الـ triggers. |
| `iroha_ivm_exec_ms` | مضيف IVM | تنبيه إذا >250 ms لكل lane | يعكس نافذة تنفيذ IVM overlays. |
| `iroha_amx_abort_total{stage}` | الـ executor | تنبيه إذا >0.05 aborts/slot أو ارتفاعات ثابتة في مرحلة واحدة | `stage`: `prepare`, `exec`, `commit`. |
| `iroha_amx_lock_conflicts_total` | مجدول AMX | تنبيه إذا >0.1 تعارضات/slot | يشير إلى مجموعات قراءات/كتابات غير دقيقة. |

## فهرس أخطاء AMX وتوقعات المشغّلين

أمثلة على الأخطاء:

- `AMX_TIMEOUT` — fragment ما تجاوز الـ budget المخصّص له
  (prepare/exec/commit).
- `AMX_LOCK_CONFLICT` — تعارض في lock تمّ اكتشافه بواسطة المجدول.
- `SETTLEMENT_ROUTER_UNAVAILABLE` — تعذّر على settlement router توجيه
  العملية داخل الـ slot.
- `PVO_MISSING` — تم توقع PVO مسبق التسجيل، لكن لم يتم العثور عليه.

يجب على المشغّلين:

- متابعة المقاييس أعلاه وإضافة لوحات (dashboards) خاصة بـ AMX إلى Grafana.
- الحفاظ على runbooks للتعامل مع ارتفاعات `AMX_TIMEOUT` أو
  `AMX_LOCK_CONFLICT` (مثل مراجعة hints الخاصة بالوصول أو ضبط الـ budgets).
- حفظ الأدلة (logs، traces، PVOs) لأي incident متعلق بـ AMX بحيث يمكن
  إعادة بنائه وتحليله offline.

## إرشادات للمطوّرين: PVOs

- تقوم Proof Verification Objects بتغليف الـ proofs الثقيلة (مثل
  الـ ZK‑proofs للـ fragments الخاصة) داخل آرتيفاكت يمكن للـ nexus host
  التحقق منه بسرعة داخل الـ slot.
- يجب على العملاء preregister الـ PVOs خارج المسار الحرج للـ slot، ثم
  الإشارة إليها من معاملات AMX عبر معرّفات مستقرة.
- يجب أن يضمن تصميم الـ PVO ما يلي:
  - تغليف Norito كاونوني (حقول versioned، ترويسة Norito، وchecksum).
  - فصل واضح بين البيانات العامة والخاصّة للحفاظ على السرّية.

لمزيد من التفاصيل حول التصميم والصيغ الدقيقة للـ PVO راجع
`docs/source/nexus.md`.

</div>

