---
lang: ur
direction: rtl
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 532c57a0dae54224af0d30640edf8a3cbc8ac9a1df7d73b563bd16c3a635aec1
source_last_modified: "2026-01-08T19:45:50.411145+00:00"
translation_last_reviewed: 2026-01-08
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_fee_model.md -->

# Nexus فیس ماڈل اپ ڈیٹس

متحدہ settlement router اب ہر lane کے لئے حتمی receipts محفوظ کرتا ہے تاکہ آپریٹرز
gas ڈیبٹس کو Nexus فیس ماڈل کے ساتھ reconcile کر سکیں۔

- router کی مکمل architecture، buffer policy، telemetry matrix، اور rollout sequencing کے لئے
  `docs/settlement-router.md` دیکھیں۔ یہ گائیڈ وضاحت کرتی ہے کہ یہاں درج پیرامیٹرز NX-3
  روڈمیپ deliverable سے کیسے جڑتے ہیں اور SREs کو پروڈکشن میں router کی نگرانی کیسے کرنی چاہیے۔
- gas asset configuration (`pipeline.gas.units_per_gas`) میں `twap_local_per_xor` decimal،
  `liquidity_profile` (`tier1`, `tier2`, یا `tier3`)، اور `volatility_class` (`stable`,
  `elevated`, `dislocated`) شامل ہیں۔ یہ flags settlement router کو feed ہوتے ہیں تاکہ
  حاصل شدہ XOR quote lane کے canonical TWAP اور haircut tier کے مطابق ہو۔
- ہر لین دین میں typed اور دستخط سے منسلک لازمی `fee_payment`
  (`FeePaymentIntent`) ہونا چاہیے۔ یہ payer کے طور پر authority یا ایک عین
  sponsor program اور اس کی ناقابل تبدیلی revision منتخب کرتا ہے، اور دستخط
  شدہ component maxima کے ساتھ ضرورت کے وقت مثبت gas bound شامل کرتا ہے۔
  پرانے میٹاڈیٹا keys `fee_sponsor`، `gas_limit` اور `gas_asset_id` مسترد ہوتے ہیں۔
- دستخط سے پہلے quote لیں: عین unsigned payload بنائیں، اس کی authority سے
  `POST /v1/fees/quote` authenticate کروائیں، تجویز کردہ intent جانچیں،
  صرف `payload.fee_payment` بدلیں، پھر اسی payload پر دستخط کر کے submit
  کریں۔ Quote reservation نہیں بلکہ observation ہے؛ admission موجودہ حالت
  دوبارہ جانچتا ہے۔
- Direct settlement authority یا ایک عین sponsor program دونوں کو قبول کرتا
  ہے۔ Receipt-lane (`lane_relay_burn`) settlement صرف عین sponsor کے لئے
  ہے: authority کی ادا کردہ Nexus fees
  `relay_capacity_unavailable` کے ساتھ مسترد ہوتی ہیں، کیونکہ authority
  balance authenticated receipt source lock نہیں ہے۔
- ہر وہ transaction جو gas ادا کرتی ہے `LaneSettlementReceipt` ریکارڈ کرتی ہے۔ ہر receipt میں
  caller فراہم کردہ source identifier، local micro-amount، فوری طور پر واجب الادا XOR، haircut کے بعد
  متوقع XOR، حاصل شدہ safety margin (`xor_variance`)، اور block timestamp ملّی سیکنڈ میں محفوظ ہوتے ہیں۔
- block execution lane/dataspace کے مطابق receipts کو aggregate کر کے `/v1/sumeragi/status` میں
  `lane_settlement_commitments` کے ذریعے شائع کرتا ہے۔ totals میں `total_local_amount`,
  `total_xor_due`, اور `total_xor_after_haircut` شامل ہیں جو بلاک پر جمع کئے جاتے ہیں
  تاکہ رات کی reconciliation exports بن سکیں۔
- نیا `total_xor_variance` کاؤنٹر یہ ٹریک کرتا ہے کہ کتنا safety margin استعمال ہوا
  (due XOR اور post-haircut expectation کے درمیان فرق)، اور `swap_metadata` deterministic conversion
  parameters (TWAP, epsilon, liquidity profile, اور volatility_class) درج کرتا ہے تاکہ auditors
  runtime configuration سے آزاد ہو کر quote inputs کی توثیق کر سکیں۔

صارفین موجودہ lane اور dataspace commitment snapshots کے ساتھ `lane_settlement_commitments` کو دیکھ سکتے ہیں
تاکہ یہ یقینی بنایا جا سکے کہ fee buffers، haircut tiers، اور swap execution ترتیب دیے گئے Nexus فیس ماڈل
کے مطابق ہیں۔

</div>
