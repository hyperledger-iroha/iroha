---
lang: ar
direction: rtl
source: docs/settlement-router.md
status: complete
translator: manual
source_hash: a00e929ef6088863db93ea122b7314ee921b9b37bea2c6e2c536082c29038f97
source_last_modified: "2025-11-12T08:35:29.309807+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/settlement-router.md (Deterministic Settlement Router) -->

# موجّه التسوية الحتمية (NX-3)

**الحالة:** مخطّط (NX-3)  
**المالكون:** Economics WG / Core Ledger WG / Treasury / SRE  
**النطاق:** يطبّق سياسة موحّدة لتحويل XOR وحجز الـ buffer كما هو موضّح في بند خارطة
الطريق «Unified lane settlement & XOR conversion (NX-3)».

يوثّق هذا المستند الخوارزمية والـ telemetry والعقود التشغيلية لموجّه التسوية
الموحّد. وهو يكمل ملخص الرسوم في `docs/source/nexus_fee_model.md` من خلال شرح كيف
يشتق الموجّه خصومات XOR، وكيف يتفاعل مع الـ DEX الكانوني من نوع AMM/CLMM (أو RFQ
محكوم كخيار احتياطي)، وكيف يقدّم إيصالات حتمية لكل lane.

## الأهداف

   العامة في Nexus أو تدفّقات AMX عبر الـ dataspace — تستخدم نفس خط أنابيب التسوية،
   بحيث تعتمد التسويات الليلية على نموذج واحد فقط.
2. **تحويل حتمي:** المبالغ المستحقّة بـ XOR مشتقة من معطيات on‑chain (متوسط TWAP،
   هامش تقلب epsilon، مستوى السيولة) وتُقرّب إلى أعلى بوحدة micro‑XOR لكي ينتج كل
   عقدة نفس الإيصال.
3. **مخازن أمان:** يحتفظ كل dataspace/lane بمخزن XOR عند مستوى P95 لـ 72 ساعة. يقوم
   الموجّه أولاً بخصم المخزن، ثم يفرض حالات Throttle أو XOR‑only عندما تهبط
   المستويات، وينشر مقاييس تمكّن المشغلين من إعادة التعبئة بشكل استباقي.
4. **سيولة محكومة:** تفضّل التحويلات الـ DEX الكانوني من نوع AMM/CLMM على lane عام،
   بينما يتم تفعيل RFQ/swap‑lines المحكومة عندما تهبط السيولة أو تُفعَّل قواطع
   الحماية (circuit breakers).

## البنية المعمارية

| المكوّن | الموقع | المسؤولية |
|--------|--------|-----------|
| `crates/settlement_router/` | حزمة الموجّه (جديدة) | حساب shadow price، تطبيق haircuts، محاسبة الـ buffer، بناء الجدول الزمني للتسويات، تتبّع تعرّض P&L. |
| `crates/oracle_adapter/` | طبقة وسيطة للأوراكل | تحافظ على نافذة TWAP (افتراضياً 60 ثانية) + تقلب متحرك، وتصدر سعر دائرة (circuit price) إذا أصبحت المصادر قديمة. |
| `crates/iroha_core/src/fees` و`LaneBlockCommitment` | سجل core | تخزين إيصالات الـ lane، وتباين XOR، وبيانات swaps داخل إثباتات الكتلة لأغراض التدقيق. |
| DEX الكانوني AMM/CLMM و RFQ محكوم | فريق DEX/AMM | تنفيذ swaps حتمية وتوفير مستويات سيولة وحلاقة (tier1=عميق → 0 bp، tier2=متوسط → 25 bp، tier3=ضعيف → 75 bp). |
| خطوط swap الخاصة بالخزانة / واجهات sponsor | Financial Services WG | تسمح لسماسرة أو مزوّدي سيولة معتمدين (≤25 % من Bmin) بإعادة تعبئة الـ buffers من دون انتظار swaps السوق. |
| الـ telemetry ولوحات المراقبة | `iroha_telemetry`، `dashboards/…` | تبث `iroha_settlement_buffer_xor` و`iroha_settlement_pnl_xor` و`iroha_settlement_haircut_bp` وغيرها؛ وتطلق لوحات المراقبة تنبيهات عند تجاوز عتبات المخازن. |

## خط أنابيب التحويل

1. **تجميع الإيصالات**
   - كل معاملة/مرحلة AMX تصدر `LaneSettlementReceipt` يحتوي على المبلغ المحلي (micro)،
     ومقدار XOR المستحق، وtier الحلاقة، والهامش المحقّق (`xor_variance_micro`) وبيانات
     الجهة المستدعية (كما في `docs/source/nexus_fee_model.md`).
   - يقوم الموجّه بتجميع الإيصالات لكل `(lane, dataspace)` أثناء تنفيذ الكتلة.

2. **سعر الظل (`shadow_price.rs`)**
   - يسحب متوسط TWAP لـ 60 ثانية (`twap_local_per_xor`) وعينة تقلب لـ 30 ثانية.
   - يحسب epsilon: أساس 25 bp + (فئة التقلب × 25 bp)، مع سقف قدره 75 bp.
   - يخزّن قيمة `volatility_class` الناتجة (`stable`، `elevated`، `dislocated`)
     داخل `LaneSwapMetadata` بحيث يستطيع المدققون ولوحات المراقبة ربط الهامش
     الإضافي بنظام الأوراكل المرتبط بـ `pipeline.gas.units_per_gas`.
   - يحسب `xor_due = ceil(local_micro / twap × (1 + epsilon))`.
   - يخزّن الثلاثي `(twap, epsilon, tier)` ضمن بيانات swap لأغراض التدقيق.

3. **الحلاقة + التباين (`haircut.rs`)**
   - يطبق حلاقة حسب ملف السيولة (tier1=0 bp، tier2=25 bp، tier3=75 bp).
   - يسجل `total_xor_after_haircut` و`total_xor_variance` (الفرق بين المبلغ
     المستحق وبعد الحلاقة) حتى تتمكن الخزانة من رؤية مقدار هامش الأمان المستهلك في
     كل tier.

4. **خصم الـ buffer (`buffer.rs`)**
   - يعرّف كل buffer لـ lane/dataspace قيمة Bmin = إنفاق XOR عند P95 على مدى 72 ساعة.
   - الحالات: **Normal** (≥75 %)، **Alert** (<75 %)، **Throttle** (<25 %)،
     **XOR‑only** (<10 %)، **Halt** (<2 % أو أوراكل متوقف/قديم).
   - يخصم الموجّه الـ buffer لكل إيصال؛ وعند تجاوز العتبات، يبث telemetry حتميًا
     ويطبّق قيودًا (مثل خفض معدل إدراج المعاملات إلى النصف عند أقل من 25 %).
   - يجب أن تعلن بيانات الـ lane الوصفية عن احتياطي الخزانة عبر:
     - `metadata.settlement.buffer_account` – معرّف الحساب الذي يحتفظ بالاحتياطي
       (مثلاً `buffer::cbdc_treasury`).
     - `metadata.settlement.buffer_asset` – الأصل المالي الذي يُخصم (عادة
       `xor#sora`).
     - `metadata.settlement.buffer_capacity_micro` – السعة بوحدة micro‑XOR (سلسلة
       عشرية).
   - تتضمّن الـ telemetry متغيرات `iroha_settlement_buffer_xor` (الهامش المتبقّي)،
     و`iroha_settlement_buffer_capacity_xor` و`iroha_settlement_buffer_state` (حالة
     متقطعة).

5. **جدولة الـ swaps (`schedule.rs`)**
   - يجمع الإيصالات بحسب الأصل المحلي وtier السيولة لتقليل عدد الـ swaps.
   - يراعي حدود الحجم/الكمية لكل أمر AMM/CLMM.
   - ينتج `LaneSwapSchedule` حتمي يصف:
     - الأصل المحلي المجمَّع؛
     - كمية XOR المستهدفة؛
     - الـ DEX/tier المختار؛
     - الحلاقة المطبقة؛
     - الـ lane/dataspace المتأثرة.

6. **التسوية وإثباتات الكتلة**
   - يرسل المُجدول أوامر إلى طبقة الـ DEX. تُشفَّر نتيجة كل swap في
     `LaneSwapMetadata` وتُلحق بـ `LaneBlockCommitment`.
   - يمكن للمدققين إعادة بناء التحويل بين XOR والعملات المحلية بالاعتماد على:
     - TWAP والتقلب؛
     - tier السيولة؛
     - الحلاقة المطبقة؛
     - التباين الكلي في XOR.

## الـ telemetry والتنبيهات

الحد الأدنى من المقاييس التي يجب نشرها:

- `iroha_settlement_buffer_xor{dataspace,lane}` – الهامش المتبقي من XOR.
- `iroha_settlement_buffer_capacity_xor{dataspace,lane}` – سعة Bmin المكوّنة.
- `iroha_settlement_buffer_state{dataspace,lane}` – حالة متقطعة (`normal`, `alert`,
  `throttle`, `xor_only`, `halt`).
- `iroha_settlement_pnl_xor{dataspace,lane}` – الربح/الخسارة المتراكمين بـ XOR منذ
  البداية.
- `iroha_settlement_haircut_bp{dataspace,lane,tier}` – الحلاقة الفعلية المطبقة.

لوحات المراقبة والتنبيهات:

- إرسال تنبيهات عندما تنتقل الحالة إلى `alert` (<75 %) أو أسوأ.
- بناء لوحات تعرض الـ headroom لكل lane/dataspace والاستهلاك اليومي.
- إبراز الـ lanes حيث يتجاوز التباين المتراكم (`total_xor_variance`) النطاق المتوقع.

## استراتيجية النشر

يُنصح بتمكين الموجّه على مراحل:

1. **وضع Shadow:** تفعيل حسابات الموجّه + الـ telemetry مع إبقاء
2. **تفعيل الخصم:** تشغيل `settlement_debit=true` للـ lanes منخفضة المخاطر (الملف C)
   والتحقق من نتائج التسوية.
3. **تفعيل التحويلات:** تشغيل شرائح TWAMM + swaps الخاصة بالـ AMM عندما تشير
   الـ telemetry إلى استقرار المخازن؛ مع إبقاء RFQ الاحتياطي جاهزًا.
4. **تكامل AMX:** التأكد من أن lanes AMX تمرّر الإيصالات وتعرض
   `SETTLEMENT_ROUTER_UNAVAILABLE` لطبقة الـ SDK للسماح بعمليات إعادة المحاولة.
   أية دمجات مستقبلية بالالتزام بأهداف الـ telemetry الخاصة بالتسوية.

اتباع هذا المستند يضمن أن مهمة NX‑3 على خارطة الطريق تقدم سلوكًا حتميًا، ومواد
قابلة للتدقيق، وإرشادات واضحة للمطورين والمشغلين على حد سواء.

</div>
