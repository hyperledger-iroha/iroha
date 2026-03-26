---
lang: ar
direction: rtl
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T15:38:30.662574+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# دليل تسوية الريبو

يوثق هذا الدليل التدفق الحتمي لاتفاقيات الريبو والريبو العكسي في Iroha.
It covers CLI orchestration, SDK helpers, and the expected governance knobs so operators can
بدء الاتفاقيات وهامشها وإلغاءها دون كتابة حمولات Norito الأولية. من أجل الحكم
راجع قوائم المراجعة والتقاط الأدلة وإجراءات الاحتيال/التراجع
[`repo_ops.md`](./repo_ops.md)، والذي يفي ببند خريطة الطريق F1.

## أوامر واجهة سطر الأوامر

يقوم الأمر `iroha app repo` بتجميع المساعدين الخاصين بالريبو:

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

* `repo initiate` و`repo unwind` يحترمان `--input/--output` وبالتالي فإن `InstructionBox` تم إنشاؤه
  يمكن نقل الحمولات إلى تدفقات CLI أخرى أو إرسالها على الفور.
* قم باجتياز `--custodian <account>` لتوجيه الضمانات إلى أمين حفظ ثلاثي الأطراف. عند حذفها،
  يتلقى الطرف المقابل التعهد مباشرة (الريبو الثنائي).
* يستعلم `repo margin` عن دفتر الأستاذ عبر `FindRepoAgreements` ويبلغ عن الهامش المتوقع التالي
  الطابع الزمني (بالملي ثانية) إلى جانب ما إذا كان رد اتصال الهامش مستحقًا حاليًا.
* يقوم `repo margin-call` بإلحاق تعليمات `RepoMarginCallIsi`، لتسجيل نقطة تفتيش الهامش و
  انبعاث الأحداث لجميع المشاركين. يتم رفض المكالمات إذا لم ينقض الإيقاع أو إذا كان
  يتم تقديم التعليمات من قبل شخص غير مشارك.

## مساعدو Python SDK

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

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="soraカタカナ..."))
draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="soraカタカナ...",
    counterparty="soraカタカナ...",
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

* يقوم كلا المساعدين بتطبيع الكميات الرقمية وحقول البيانات الوصفية قبل استدعاء روابط PyO3.
* يعكس `RepoAgreementRecord` حساب جدول وقت التشغيل حتى تتمكن الأتمتة خارج دفتر الأستاذ من
  تحديد موعد استحقاق عمليات رد الاتصال دون إعادة حساب الإيقاع يدويًا.

## تسويات DvP / PvP

يقوم الأمر `iroha app settlement` بمراحل تعليمات التسليم مقابل الدفع والدفع مقابل الدفع:

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

* تقبل كميات الساق القيم المتكاملة أو العشرية ويتم التحقق من صحتها مقابل دقة الأصل.
* يقبل `--atomicity` `all-or-nothing`، أو `commit-first-leg`، أو `commit-second-leg`. استخدم هذه الأوضاع
  مع `--order` للتعبير عن المحطة التي تظل ملتزمة في حالة فشل المعالجة اللاحقة (`commit-first-leg`
  يحافظ على تطبيق الساق الأولى. `commit-second-leg` يحتفظ بالثانية).
* تُصدر استدعاءات واجهة سطر الأوامر (CLI) بيانات تعريف تعليمات فارغة اليوم؛ استخدم مساعدي Python عند مستوى التسوية
  يجب إرفاق البيانات الوصفية.
* راجع [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) للتعرف على تعيين حقل ISO 20022 الذي
  يدعم هذه التعليمات (`sese.023`، `sese.025`، `colr.007`، `pacs.009`، `camt.054`).
* قم بتمرير `--iso-xml-out <path>` حتى تقوم واجهة سطر الأوامر (CLI) بإصدار معاينة XML أساسية إلى جانب Norito
  تعليمات؛ يتبع الملف التعيين أعلاه (`sese.023` لـ DvP، `sese.025` لـ PvP`). إقران
  ضع علامة على `--iso-reference-crosswalk <path>` حتى تتحقق واجهة سطر الأوامر (CLI) من `--delivery-instrument-id` مقابل
  نفس اللقطة التي يستخدمها Torii أثناء قبول وقت التشغيل.

يعكس مساعدو Python سطح CLI:

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    TransactionConfig,
    TransactionDraft,
)

draft = TransactionDraft(TransactionConfig(chain_id="dev-chain", authority="soraカタカナ..."))
delivery = SettlementLeg(
    asset_definition_id="4fEiy2n5VMFVfi6BzDJge519zAzg",
    quantity="10",
    from_account="soraカタカナ...",
    to_account="soraカタカナ...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
    quantity="1000",
    from_account="soraカタカナ...",
    to_account="soraカタカナ...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
        quantity="500",
        from_account="soraカタカナ...",
        to_account="soraカタカナ...",
    ),
    SettlementLeg(
        asset_definition_id="5tPkFK6s2zUcd1qUHyTmY7fDVa2n",
        quantity="460",
        from_account="soraカタカナ...",
        to_account="soraカタカナ...",
    ),
)
```

## توقعات الحتمية والحوكمة

تعتمد تعليمات الريبو حصريًا على الأنواع الرقمية المشفرة بـ Norito والأنواع المشتركة
منطق `RepoGovernance::with_defaults`. ضع الثوابت التالية في الاعتبار:* يتم تسلسل الكميات بقيم حتمية `NumericSpec`: استخدام الساقين النقدية
  `fractional(2)` (منزلتان عشريتان)، وتستخدم الجوانب الجانبية `integer()`. لا تقدم
  القيم بدقة أكبر - سيرفضها حراس وقت التشغيل وسيتباعد النظراء.
* تحتفظ عمليات إعادة الشراء ثلاثية الأطراف بمعرف حساب الوصي في `RepoAgreement`. دورة الحياة وأحداث الهامش
  قم بإصدار حمولة `RepoAccountRole::Custodian` حتى يتمكن الأمناء من الاشتراك وتسوية المخزون.
* يتم تثبيت قصات الشعر بسرعة 10000 بت في الثانية (100%) وترددات الهامش هي ثوانٍ كاملة. تقديم
  معلمات الحوكمة في تلك الوحدات الأساسية لتظل متوافقة مع توقعات وقت التشغيل.
* الطوابع الزمنية تكون دائمًا بالمللي ثانية. يقوم كافة المساعدين بإعادة توجيهها دون تغيير إلى Norito
  الحمولة بحيث يستنتج الأقران جداول زمنية متطابقة.
* تعليمات البدء والإلغاء تعيد استخدام نفس معرف الاتفاقية. وقت التشغيل يرفض
  معرفات مكررة وإلغاء اتفاقيات غير معروفة؛ يقوم مساعدو CLI/SDK بإظهار هذه الأخطاء مبكرًا.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` يُرجع الإيقاع الأساسي. دائما
  راجع هذه اللقطة قبل تشغيل عمليات الاسترجاعات لتجنب إعادة تشغيل الجداول القديمة.