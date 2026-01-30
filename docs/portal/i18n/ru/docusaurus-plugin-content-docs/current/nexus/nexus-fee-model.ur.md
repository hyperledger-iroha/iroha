---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-fee-model
title: Nexus فیس ماڈل اپ ڈیٹس
description: `docs/source/nexus_fee_model.md` کا آئینہ، جو lane settlement receipts اور reconciliation surfaces کی دستاویز کرتا ہے۔
---

:::note کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_fee_model.md` کی عکاسی کرتا ہے۔ جاپانی، عبرانی، ہسپانوی، پرتگالی، فرانسیسی، روسی، عربی اور اردو ترجمے migrate ہونے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Nexus فیس ماڈل اپ ڈیٹس

یکساں settlement router اب ہر lane کے لئے deterministic receipts محفوظ کرتا ہے تاکہ آپریٹرز gas debits کو Nexus فیس ماڈل کے مطابق reconcile کر سکیں۔

- router کی مکمل architecture، buffer policy، telemetry matrix اور rollout sequencing کے لئے `docs/settlement-router.md` دیکھیں۔ یہ گائیڈ وضاحت کرتا ہے کہ یہاں درج parameters کیسے NX-3 roadmap deliverable سے جڑتے ہیں اور SREs کو production میں router کی نگرانی کیسے کرنی چاہئے۔
- Gas asset configuration (`pipeline.gas.units_per_gas`) میں `twap_local_per_xor` decimal، `liquidity_profile` (`tier1`, `tier2`, یا `tier3`) اور `volatility_class` (`stable`, `elevated`, `dislocated`) شامل ہیں۔ یہ flags settlement router کو feed ہوتے ہیں تاکہ حاصل ہونے والی XOR quote، canonical TWAP اور lane کے haircut tier سے میل کھائے۔
- ہر gas payment والی transaction ایک `LaneSettlementReceipt` ریکارڈ کرتی ہے۔ ہر receipt caller کی فراہم کردہ source identifier، local micro-amount، فوری واجب الادا XOR، haircut کے بعد expected XOR، حاصل شدہ variance (`xor_variance_micro`) اور block timestamp (milliseconds) محفوظ کرتا ہے۔
- Block execution receipts کو lane/dataspace کے حساب سے aggregate کرتا ہے اور انہیں `/v1/sumeragi/status` میں `lane_settlement_commitments` کے ذریعے شائع کرتا ہے۔ totals میں `total_local_micro`, `total_xor_due_micro`, اور `total_xor_after_haircut_micro` شامل ہوتے ہیں جو block پر جمع کر کے nightly reconciliation exports کے لئے فراہم ہوتے ہیں۔
- ایک نیا `total_xor_variance_micro` counter یہ track کرتا ہے کہ کتنا safety margin استعمال ہوا (due XOR اور post-haircut expectation کے درمیان فرق)، اور `swap_metadata` deterministic conversion parameters (TWAP, epsilon, liquidity profile, اور volatility_class) کو دستاویز کرتا ہے تاکہ auditors runtime configuration سے الگ quote inputs کی تصدیق کر سکیں۔

Consumers `lane_settlement_commitments` کو موجودہ lane اور dataspace commitment snapshots کے ساتھ دیکھ سکتے ہیں تاکہ یہ تصدیق ہو کہ fee buffers، haircut tiers، اور swap execution configured Nexus fee model سے میل کھاتے ہیں۔
