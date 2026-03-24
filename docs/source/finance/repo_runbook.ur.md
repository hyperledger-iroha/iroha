---
lang: ur
direction: rtl
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T15:38:30.662574+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ریپو تصفیہ رن بک

یہ گائیڈ Iroha میں ریپو اور ریورس ریپو معاہدوں کے لئے عین مطابق بہاؤ کی دستاویز کرتا ہے۔
اس میں سی ایل آئی آرکیسٹریشن ، ایس ڈی کے مددگار ، اور متوقع گورننس نوبس شامل ہیں تاکہ آپریٹرز کر سکتے ہیں
خام Norito پے لوڈ لکھے بغیر ، مارجن اور غیر منحصر معاہدوں کا آغاز کریں۔ گورننس کے لئے
چیک لسٹس ، شواہد کی گرفتاری ، اور دھوکہ دہی/رول بیک طریقہ کار دیکھیں
.

## سی ایل آئی کمانڈز

`iroha app repo` کمانڈ گروپس ریپو مخصوص مددگار:

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator i105... \
  --counterparty i105... \
  --custodian i105... \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1000 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1050 \
  --rate-bps 250 \
  --maturity-timestamp-ms 1704000000000 \
  --haircut-bps 1500 \
  --margin-frequency-secs 86400

# Generate the unwind leg
iroha --config client.toml --output \
  repo unwind \
  --agreement-id daily_repo \
  --initiator i105... \
  --counterparty i105... \
  --cash-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --cash-quantity 1005 \
  --collateral-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` اور `repo unwind` احترام `--input/--output` لہذا پیدا شدہ `InstructionBox`
  پے لوڈ کو دوسرے CLI بہاؤ میں پائپ کیا جاسکتا ہے یا فوری طور پر جمع کرایا جاسکتا ہے۔
* `--custodian <account>` کو ایک سہ فریقی کسٹوڈین کے لئے خودکش حملہ کرنے کے لئے پاس کریں۔ جب چھوڑ دیا جائے ،
  ہم منصب کو براہ راست عہد حاصل کرتا ہے (دو طرفہ ریپو)۔
* `repo margin` `FindRepoAgreements` کے ذریعے لیجر سے استفسار کرتا ہے اور اگلے متوقع مارجن کی اطلاع دیتا ہے
  ٹائم اسٹیمپ (ملی سیکنڈ میں) اس کے ساتھ ساتھ کہ آیا اس وقت مارجن کال بیک ہے یا نہیں۔
* `repo margin-call` ایک `RepoMarginCallIsi` ہدایت کو شامل کرتا ہے ، مارجن چوکی کو ریکارڈ کرتا ہے اور
  تمام شرکاء کے لئے واقعات کا اخراج۔ کالوں کو مسترد کردیا جاتا ہے اگر کیڈینس میں اضافہ نہیں ہوا ہے یا اگر
  ہدایات غیر شریک کے ذریعہ پیش کی جاتی ہیں۔

## ازگر ایس ڈی کے مددگار

```python
from iroha_python import (
    create_torii_client,
    RepoAgreementRecord,
    RepoCashLeg,
    RepoCollateralLeg,
    RepoGovernance,
    TransactionConfig,
    TransactionDraft,
)

client = create_torii_client("client.toml")

cash = RepoCashLeg(asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="1050",
    metadata={"isin": "ABC123"},
)
governance = RepoGovernance(haircut_bps=1500, margin_frequency_secs=86_400)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="i105..."))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="i105...",
    counterparty="i105...",
    cash_leg=cash,
    collateral_leg=collateral,
    rate_bps=250,
    maturity_timestamp_ms=1_704_000_000_000,
    governance=governance,
)
# ... additional instructions ...
envelope = draft.sign_with_keypair(my_keypair)
client.submit_transaction_envelope(envelope)

# Margin schedule
agreements = client.list_repo_agreements()
record = RepoAgreementRecord.from_payload(agreements[0])
next_margin = record.next_margin_check_after(at_timestamp_ms=now_ms)
```

* دونوں مددگار PYO3 پابندیوں سے پہلے عددی مقدار اور میٹا ڈیٹا فیلڈز کو معمول پر لاتے ہیں۔
* `RepoAgreementRecord` رن ٹائم شیڈول کے حساب کتاب کو آئینہ دیتا ہے لہذا آف لیجر آٹومیشن کر سکتا ہے
  اس بات کا تعین کریں کہ کال بیکس کو دستی طور پر کیڈینس کی بحالی کے بغیر کب باقی ہیں۔

## ڈی وی پی / پی وی پی بستیوں

`iroha app settlement` کمانڈ ڈلیوری بمقابلہ ادائیگی اور ادائیگی کے مقابلے میں ادائیگی کی ہدایات:

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from i105... \
  --delivery-to i105... \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from i105... \
  --payment-to i105... \
  --order payment-then-delivery \
  --atomicity all-or-nothing \
  --iso-reference-crosswalk /opt/iso/isin_crosswalk.json \
  --iso-xml-out trade_dvp.xml

# Cross-currency swap (payment-versus-payment)
iroha --config client.toml --output \
  settlement pvp \
  --settlement-id trade_pvp \
  --primary-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --primary-quantity 500 \
  --primary-from i105... \
  --primary-to i105... \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from i105... \
  --counter-to i105... \
  --iso-xml-out trade_pvp.xml
```

* ٹانگوں کی مقدار لازمی یا اعشاریہ اقدار کو قبول کرتی ہے اور اثاثوں کی صحت سے متعلق کے خلاف توثیق کی جاتی ہے۔
* `--atomicity` `all-or-nothing` ، `commit-first-leg` ، یا `commit-second-leg` کو قبول کرتا ہے۔ ان طریقوں کا استعمال کریں
  `--order` کے ساتھ اظہار کرنے کے لئے کہ اگر اس کے بعد کی پروسیسنگ میں ناکام ہوجاتا ہے تو (`commit-first-leg`
  پہلی ٹانگ کا اطلاق ہوتا ہے۔ `commit-second-leg` دوسرے کو برقرار رکھتا ہے)۔
* سی ایل آئی کی درخواستیں آج خالی انسٹرکشن میٹا ڈیٹا کا اخراج کرتی ہیں۔ تصفیہ کی سطح پر جب ازگر کے مددگاروں کا استعمال کریں
  میٹا ڈیٹا کو منسلک کرنے کی ضرورت ہے۔
* آئی ایس او 20022 فیلڈ میپنگ کے لئے [`settlement_iso_mapping.md`] (./settlement_iso_mapping.md) دیکھیں
  ان ہدایات کی پشت پناہی (`sese.023` ، `sese.025` ، `colr.007` ، `pacs.009` ، `camt.054`)۔
Norito کے ساتھ ساتھ CLI کو ایک کیننیکل XML پیش نظارہ خارج کرنے کے لئے `--iso-xml-out <path>` پاس کریں
  ہدایت ؛ فائل مذکورہ بالا نقشہ سازی کی پیروی کرتی ہے (DVP کے لئے `sese.023` ، PVP` کے لئے `sese.025`)۔ جوڑا
  `--iso-reference-crosswalk <path>` کے ساتھ پرچم ہے لہذا CLI `--delivery-instrument-id` کے خلاف تصدیق کرتا ہے
  رن ٹائم داخلہ کے دوران ایک ہی سنیپ شاٹ Torii استعمال کرتا ہے۔

ازگر کے مددگار سی ایل آئی کی سطح کی عکسبندی کرتے ہیں:

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="i105..."))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="i105...",
    to_account="i105...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="i105...",
    to_account="i105...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="i105...",
        to_account="i105...",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="i105...",
        to_account="i105...",
    ),
)
```

## عزم اور حکمرانی کی توقعات

ریپو ہدایات خصوصی طور پر Norito- انکوڈڈ عددی اقسام اور مشترکہ پر انحصار کرتی ہیں
`RepoGovernance::with_defaults` منطق۔ مندرجہ ذیل حملہ آوروں کو ذہن میں رکھیں:* مقدار کو عین مطابق `NumericSpec` اقدار کے ساتھ سیریلائز کیا جاتا ہے: نقد ٹانگوں کا استعمال
  `fractional(2)` (دو اعشاریہ مقامات) ، کولیٹرل ٹانگیں `integer()` استعمال کریں۔ جمع نہ کریں
  زیادہ سے زیادہ صحت سے متعلق اقدار - رنٹائم گارڈز انہیں مسترد کردیں گے اور ساتھیوں کو موڑ دیا جائے گا۔
* ٹری پارٹی ریپوز `RepoAgreement` میں کسٹوڈین اکاؤنٹ کی شناخت برقرار رکھتے ہیں۔ لائف سائیکل اور مارجن واقعات
  ایک `RepoAccountRole::Custodian` پے لوڈ کا اخراج کریں تاکہ حراستی افراد انوینٹری کو سبسکرائب اور مصالحت کرسکیں۔
* بال کٹوانے کو 10000bps (100 ٪) پر کلیمپ کیا جاتا ہے اور مارجن تعدد پورے سیکنڈ میں ہوتا ہے۔ فراہم کریں
  رن ٹائم توقعات کے ساتھ منسلک رہنے کے لئے ان کیننیکل یونٹوں میں گورننس پیرامیٹرز۔
* ٹائم اسٹیمپ ہمیشہ یونکس ملی سیکنڈ ہوتے ہیں۔ تمام مددگار ان کو بغیر کسی تبدیلی کو Norito میں بھیج دیتے ہیں
  پے لوڈ تو ہم عمر ایک جیسے نظام الاوقات اخذ کرتے ہیں۔
* ابتداء اور کھولنے والی ہدایات ایک ہی معاہدے کی شناخت کنندہ کو دوبارہ استعمال کرتی ہیں۔ رن ٹائم مسترد کرتا ہے
  نامعلوم معاہدوں کے لئے ڈپلیکیٹ آئی ڈی اور انوائڈز۔ سی ایل آئی/ایس ڈی کے مددگار ان غلطیوں کو جلد پیش کرتے ہیں۔
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` کیننیکل کیڈینس کو واپس کریں۔ ہمیشہ
  باسی نظام الاوقات کو دوبارہ چلانے سے بچنے کے لئے کال بیکس کو متحرک کرنے سے پہلے اس سنیپ شاٹ سے مشورہ کریں۔