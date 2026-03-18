---
lang: ur
direction: rtl
source: docs/settlement-router.md
status: complete
translator: manual
source_hash: a00e929ef6088863db93ea122b7314ee921b9b37bea2c6e2c536082c29038f97
source_last_modified: "2025-11-12T08:35:29.309807+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/settlement-router.md (Deterministic Settlement Router) -->

# ڈیٹرمنسٹک سیٹلمنٹ روٹر (NX-3)

**اسٹیٹس:** پلان کیا گیا (NX-3)  
**مالکان:** Economics WG / Core Ledger WG / Treasury / SRE  
**اسکوپ:** روڈ میپ آئٹم “Unified lane settlement & XOR conversion (NX-3)” میں بیان کی
گئی XOR کنورژن اور buffer پالیسی کو نافذ کرتا ہے۔

یہ دستاویز متحدہ سیٹلمنٹ روٹر کے الگورتھم، ٹیلی میٹری اور آپریشنل معاہدوں (contracts)
کو واضح کرتی ہے۔ یہ `docs/source/nexus_fee_model.md` میں موجود فیس سمری کی تکمیل کرتی
ہے، اور بتاتی ہے کہ روٹر XOR debits کیسے derive کرتا ہے، canonical AMM/CLMM DEX (یا
governed RFQ fallback) کے ساتھ کیسے بات چیت کرتا ہے، اور ہر lane کے لیے deterministic
receipts کیسے فراہم کرتا ہے۔

## اہداف

   cross‑DS فلو — ایک ہی سیٹلمنٹ pipeline استعمال کریں تاکہ nightly reconciliation کے
   لیے صرف ایک ماڈل درکار ہو۔
2. **ڈیٹرمنسٹک کنورژن:** XOR میں واجب الادا رقم، on‑chain پیرامیٹرز (TWAP، volatility
   epsilon، liquidity tier) سے derive ہوتی ہے اور micro‑XOR تک اوپر کی سمت میں
   round ہوتی ہے، تاکہ ہر node ایک جیسا receipt بنائے۔
3. **سکیورٹی buffers:** ہر dataspace/lane، XOR کا 72 گھنٹے P95 buffer رکھتا ہے۔ روٹر
   سب سے پہلے buffer سے debit کرتا ہے، سطحیں گرنے پر throttle/XOR‑only gates لگاتا ہے،
   اور ایسی metrics شائع کرتا ہے جس کی مدد سے operators پیشگی refill کر سکیں۔
4. **گورن شدہ liquidity:** کنورژن کے لیے ترجیح canonical public‑lane AMM/CLMM DEX کو
   دی جاتی ہے؛ جب depth کم ہو یا circuit breakers ٹرِگر ہوں تو governed RFQ/swap‑line
   fallbacks استعمال کیے جاتے ہیں۔

## آرکیٹیکچر

| جزو | مقام | ذمّہ داری |
|-----|------|-----------|
| `crates/settlement_router/` | روٹر crate (نیا) | shadow price کیلکولیشن، haircuts، buffer اکاؤنٹنگ، schedule بلڈر، P&L exposure ٹریکنگ۔ |
| `crates/oracle_adapter/` | اوپری لیئر برائے oracle | TWAP ونڈو (ڈیفالٹ 60 سیکنڈ) + رولنگ volatility کو برقرار رکھتا ہے اور اگر feeds پرانی ہو جائیں تو circuit price جاری کرتا ہے۔ |
| `crates/iroha_core/src/fees` اور `LaneBlockCommitment` | core ledger | lane receipts، XOR variance اور swap metadata کو بلاک پروف میں ذخیرہ کرتا ہے تاکہ auditors ان کا جائزہ لے سکیں۔ |
| canonical AMM/CLMM DEX اور governed RFQ | DEX/AMM ٹیم | deterministic swaps چلاتا ہے اور depth + haircut tiers فراہم کرتا ہے (tier1=deep → 0 bp، tier2=mid → 25 bp، tier3=thin → 75 bp)۔ |
| Treasury swap lines / sponsor APIs | Financial Services WG | white‑listed sponsors یا MM credit lines (≤25 % Bmin) کو اجازت دیتے ہیں کہ وہ market swaps کا انتظار کیے بغیر buffers کو refill کر سکیں۔ |
| ٹیلی میٹری + ڈیش بورڈز | `iroha_telemetry`, `dashboards/…` | `iroha_settlement_buffer_xor`, `iroha_settlement_pnl_xor`, `iroha_settlement_haircut_bp` وغیرہ شائع کرتا ہے؛ dashboards buffer thresholds پر alerts بھیجتے ہیں۔ |

## کنورژن کا pipeline

1. **Receipts ایگری گیشن**
   - ہر ٹرانزیکشن/AMX leg ایک `LaneSettlementReceipt` خارج کرتی ہے، جس میں مقامی micro
     اماؤنٹ، XOR due، haircut tier، realised variance (`xor_variance_micro`) اور caller
     metadata شامل ہوتے ہیں (جیسا کہ `docs/source/nexus_fee_model.md` میں بیان ہے)۔
   - روٹر بلاک execution کے دوران receipts کو `(lane, dataspace)` کی بنیاد پر گروپ کرتا
     ہے۔

2. **Shadow price (`shadow_price.rs`)**
   - 60‑سیکنڈ TWAP (`twap_local_per_xor`) اور 30‑سیکنڈ volatility sample حاصل کرتا ہے۔
   - epsilon = بنیادی 25 bp + (volatility bucket × 25 bp، زیادہ سے زیادہ 75 bp) کیلکولیٹ
     کرتا ہے۔
   - resolved `volatility_class` (`stable`, `elevated`, `dislocated`) کو
     `LaneSwapMetadata` میں اسٹور کرتا ہے تاکہ auditors اور dashboards اضافی margin کو
     oracle regime کے ساتھ ملا سکیں جو `pipeline.gas.units_per_gas` میں capture ہوتا ہے۔
   - `xor_due = ceil(local_micro / twap × (1 + epsilon))` کیلکولیٹ کرتا ہے۔
   - triple `(twap, epsilon, tier)` کو audit کے لیے swap metadata میں محفوظ کرتا ہے۔

3. **Haircut + variance (`haircut.rs`)**
   - liquidity profile کے مطابق haircuts لگاتا ہے (tier1=0 bp، tier2=25 bp، tier3=75 bp)۔
   - `total_xor_after_haircut` اور `total_xor_variance` (due اور post‑haircut کے فرق)
     کو ریکارڈ کرتا ہے تاکہ Treasury دیکھ سکے کہ حفاظتی margin میں سے کتنا ہر tier
     استعمال کر رہا ہے۔

4. **Buffer سے debit (`buffer.rs`)**
   - ہر lane/dataspace buffer کے لیے Bmin = 72 گھنٹے P95 XOR spend define کیا جاتا ہے۔
   - حالتیں: **Normal** (≥75 %)، **Alert** (<75 %)، **Throttle** (<25 %)،
     **XOR‑only** (<10 %)، **Halt** (<2 % یا oracle stale)۔
   - روٹر ہر receipt کے بدلے buffer سے debit کرتا ہے؛ thresholds کراس ہونے پر
     deterministic telemetry شائع کرتا ہے اور throttles نافذ کرتا ہے (مثلاً 25 % سے
     نیچے inclusion rate کو آدھا کر دینا)۔
   - lane metadata میں Treasury reserve کے لیے درج ذیل فیلڈز لازماً کنفیگر ہوں:
     - `metadata.settlement.buffer_account` – وہ Account ID جو reserve رکھتا ہے
       (مثلاً `buffer::cbdc_treasury`)۔
     - `metadata.settlement.buffer_asset` – وہ Asset جس سے headroom debit ہوتا ہے
       (عام طور پر `xor#sora`)۔
     - `metadata.settlement.buffer_capacity_micro` – micro‑XOR میں capacity (decimal
       سٹرنگ)۔
   - ٹیلی میٹری `iroha_settlement_buffer_xor` (باقی headroom)،
     `iroha_settlement_buffer_capacity_xor` اور `iroha_settlement_buffer_state` (ڈسکریٹ
     اسٹیٹ) کو surface کرتی ہے۔

5. **Swaps کی scheduling (`schedule.rs`)**
   - مقامی asset اور liquidity tier کی بنیاد پر receipts کو گروپ کرتا ہے تاکہ swaps
     کی تعداد کم سے کم رہے۔
   - ہر AMM/CLMM order کے لیے سائز/کوانٹیٹی کی پابندیوں کا احترام کرتا ہے۔
   - deterministic `LaneSwapSchedule` بناتا ہے، جس میں مندرجہ ذیل معلومات ہوتی ہیں:
     - aggregated لوکل asset؛
     - XOR میں target رقم؛
     - منتخب DEX/tier؛
     - اپلائی ہونے والا haircut؛
     - متاثرہ lane/dataspace۔

6. **سیٹلمنٹ اور بلاک پروفز**
   - scheduler، DEX layer کو orders بھیجتا ہے۔ ہر swap کا نتیجہ `LaneSwapMetadata` میں encode
     ہوتا ہے اور `LaneBlockCommitment` کے ساتھ جوڑا جاتا ہے۔
   - Auditors، TWAP، volatility، liquidity tier، اپلائی ہونے والے haircut اور
     مجموعی XOR variance کی مدد سے XOR‑to‑local conversion کو دوبارہ reconstruct کر
     سکتے ہیں۔

## ٹیلی میٹری اور alerts

کم از کم درج ذیل metrics شائع ہونی چاہئیں:

- `iroha_settlement_buffer_xor{dataspace,lane}` – XOR میں باقی headroom۔
- `iroha_settlement_buffer_capacity_xor{dataspace,lane}` – کنفیگرڈ Bmin capacity۔
- `iroha_settlement_buffer_state{dataspace,lane}` – discrete state (`normal`, `alert`,
  `throttle`, `xor_only`, `halt`)۔
- `iroha_settlement_pnl_xor{dataspace,lane}` – XOR میں accumulated P&L۔
- `iroha_settlement_haircut_bp{dataspace,lane,tier}` – effective haircut (basis points)۔

ڈیش بورڈز اور alerts:

- جب state `alert` (<75 %) یا اس سے بدتر میں جائے تو alert جاری کریں۔
- ایسے پینلز بنائیں جو lane/dataspace کے لحاظ سے headroom اور روزانہ کا consumption
  دکھائیں۔
- ان lanes کو نمایاں کریں جہاں cumulative variance (`total_xor_variance`) متوقع رینج
  سے باہر چلا جائے۔

## ڈپلائمنٹ حکمتِ عملی

روٹر کو phases میں enable کرنے کی سفارش کی جاتی ہے:

1. **Shadow mode:** روٹر کے حسابات + telemetry کو enable کریں، مگر
2. **Debit enablement:** `settlement_debit=true` کو کم خطرے والے lanes (پروفائل C) کے
   لیے فعال کریں اور reconciliation کو verify کریں۔
3. **Conversions on:** جب telemetry سے buffers کا استحکام نظر آئے تو TWAMM slices +
   AMM swaps کو فعال کریں؛ RFQ fallback کو standby پر رکھیں۔
4. **AMX انٹیگریشن:** اس بات کو یقینی بنائیں کہ AMX lanes receipts کو propagate کریں
   اور error کی صورت میں SDKs کے لیے `SETTLEMENT_ROUTER_UNAVAILABLE` surface کریں، تاکہ
   retries ممکن ہوں۔
   مستقبل کے merges کو سیٹلمنٹ telemetry کے thresholds پر پابند کریں۔

اس دستاویز پر عمل کرنے سے یقینی ہوتا ہے کہ NX‑3 روڈ میپ ٹاسک ڈیٹرمنسٹک رویہ، قابل
audit آرٹی فیکٹس، اور ڈیولپرز اور operators دونوں کے لیے واضح گائیڈ فراہم کرے۔

</div>
