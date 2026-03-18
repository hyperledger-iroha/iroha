---
lang: mn
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T16:26:46.568155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Repo Settlement Runbook

Энэхүү гарын авлага нь Iroha дахь репо болон урвуу репо хэлцлийн тодорхойлогч урсгалыг баримтжуулж байна.
Энэ нь CLI зохион байгуулалт, SDK туслахууд болон хүлээгдэж буй засаглалын товчлууруудыг хамардаг тул операторууд
Norito түүхий ачааг бичихгүйгээр гэрээг эхлүүлэх, ахиулах, цуцлах. Засаглалын хувьд
шалгах хуудас, нотлох баримтыг олж авах, залилан хийх/буцах журмуудыг үзнэ үү
[`repo_ops.md`](./repo_ops.md) нь замын зургийн F1 зүйлд нийцдэг.

## CLI тушаалууд

`iroha app repo` команд нь репо тусгай туслахуудыг бүлэглэдэг:

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator i105... \
  --counterparty i105... \
  --custodian i105... \
  --cash-asset usd#wonderland \
  --cash-quantity 1000 \
  --collateral-asset bond#wonderland \
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
  --cash-asset usd#wonderland \
  --cash-quantity 1005 \
  --collateral-asset bond#wonderland \
  --collateral-quantity 1055 \
  --settlement-timestamp-ms 1704086400000

# Inspect the next margin checkpoint for an active agreement
iroha --config client.toml repo margin --agreement-id daily_repo

# Trigger a margin call when cadence elapses
iroha --config client.toml repo margin-call --agreement-id daily_repo
```

* `repo initiate` ба `repo unwind` нь `--input/--output`-г хүндэтгэдэг тул үүсгэсэн `InstructionBox`
  Ачаа ачааллыг бусад CLI урсгал руу дамжуулах эсвэл нэн даруй илгээх боломжтой.
* Барьцаа хөрөнгийг гурван талт кастодиан руу чиглүүлэхийн тулд `--custodian <account>` дугаарыг дамжуулна уу. орхигдуулсан үед, the
  эсрэг тал нь барьцааг шууд хүлээн авдаг (хоёр талын репо).
* `repo margin` нь `FindRepoAgreements`-ээр дамжуулан дэвтэрээс асууж, дараагийн хүлээгдэж буй зөрүүг мэдээлдэг.
  цагийн тэмдэг (миллисекундээр) -ийн хажуугаар маржин дахин дуудагдах хугацаа байгаа эсэх.
* `repo margin-call` нь `RepoMarginCallIsi` зааврыг хавсаргаж, маржин шалгах цэгийг бүртгэж,
  бүх оролцогчдод зориулсан үйл явдлуудыг ялгаруулах. Хэрэв дуудлагууд дуусаагүй эсвэл дуусаагүй бол дуудлагаас татгалзана
  зааврыг оролцогч бус хүн ирүүлсэн.

## Python SDK туслахууд

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

cash = RepoCashLeg(asset_definition_id="usd#wonderland", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="bond#wonderland",
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

* Туслагч хоёулаа PyO3 холболтыг дуудахаасаа өмнө тоон хэмжигдэхүүн болон мета өгөгдлийн талбаруудыг хэвийн болгодог.
* `RepoAgreementRecord` нь ажлын цагийн хуваарийн тооцоог тусгадаг тул бүртгэлээс гадуурх автоматжуулалт нь
  Каденцыг гараар дахин тооцоолохгүйгээр буцаан дуудагдах хугацааг тодорхойлох.

## DvP / PvP тооцоо

`iroha app settlement` команд нь хүргэлт-төлбөр болон төлбөрийн эсрэг-төлбөрийн зааврыг үе шаттай:

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset bond#wonderland \
  --delivery-quantity 10 \
  --delivery-from i105... \
  --delivery-to i105... \
  --delivery-instrument-id US0378331005 \
  --payment-asset usd#wonderland \
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
  --primary-asset usd#wonderland \
  --primary-quantity 500 \
  --primary-from i105... \
  --primary-to i105... \
  --counter-asset eur#wonderland \
  --counter-quantity 460 \
  --counter-from i105... \
  --counter-to i105... \
  --iso-xml-out trade_pvp.xml
```

* Хөлийн хэмжигдэхүүнүүд нь интеграл эсвэл аравтын бутархай утгыг хүлээн авдаг бөгөөд хөрөнгийн нарийвчлалын дагуу баталгаажуулдаг.
* `--atomicity` нь `all-or-nothing`, `commit-first-leg`, эсвэл `commit-second-leg`-ийг хүлээн авдаг. Эдгээр горимуудыг ашигла
  Дараагийн боловсруулалт амжилтгүй болвол аль хөл нь хэвээр үлдэхийг илэрхийлэхийн тулд `--order` (`commit-first-leg`)
  эхний хөлийг хэрэглэсэн хэвээр байлгах; `commit-second-leg` хоёр дахь нь хадгалагдана).
* CLI дуудлагууд өнөөдөр хоосон зааварчилгааны мета өгөгдлийг ялгаруулдаг; төлбөр тооцооны түвшний үед Python туслахуудыг ашиглах
  мета өгөгдлийг хавсаргах шаардлагатай.
* ISO 20022 талбарын зураглалыг [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) харна уу.
  эдгээр зааврыг дэмждэг (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* CLI нь Norito-ийн хажууд каноник XML урьдчилан харахыг гаргахын тулд `--iso-xml-out <path>`-г дамжуулаарай.
  зааварчилгаа; файл дээрх зураглалын дагуу (DvP-д `sese.023`, PvP`-д `sese.025`). -ийг хослуул
  `--iso-reference-crosswalk <path>` дарцагтай тул CLI нь `--delivery-instrument-id`-г баталгаажуулдаг.
  Torii нь ажиллах цагийн элсэлтийн үед ашигладаг ижил хормын хувилбар.

Python туслахууд CLI гадаргууг толин тусгал болгодог:

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
    asset_definition_id="bond#wonderland",
    quantity="10",
    from_account="i105...",
    to_account="i105...",
    metadata={"isin": "ABC123"},
)
payment = SettlementLeg(
    asset_definition_id="usd#wonderland",
    quantity="1000",
    from_account="i105...",
    to_account="i105...",
)
plan = SettlementPlan(order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY)

draft.settlement_dvp("trade_dvp", delivery, payment, plan=plan, metadata={"desk": "rates"})
draft.settlement_pvp(
    "trade_pvp",
    SettlementLeg(
        asset_definition_id="usd#wonderland",
        quantity="500",
        from_account="i105...",
        to_account="i105...",
    ),
    SettlementLeg(
        asset_definition_id="eur#wonderland",
        quantity="460",
        from_account="i105...",
        to_account="i105...",
    ),
)
```

## Детерминизм ба засаглалын хүлээлт

Репо заавар нь зөвхөн Norito кодлогдсон тоон төрлүүд болон хуваалцсан
`RepoGovernance::with_defaults` логик. Дараах инвариантуудыг санаарай.* Хэмжигдэхүүнийг `NumericSpec` тодорхойлогч утгуудаар цуваа болгосон: бэлэн мөнгөний хөл ашиглах
  `fractional(2)` (аравтын хоёр орон), барьцааны хөл нь `integer()` ашигладаг. Битгий ирүүл
  илүү нарийвчлалтай утгууд—ажиллах үеийн хамгаалагчид тэдгээрээс татгалзаж, үе тэнгийнхэн нь зөрөх болно.
* Гурван талын репо нь `RepoAgreement` дахь кастодианы дансны ID-г хадгалдаг. Амьдралын мөчлөг ба захын үйл явдлууд
  `RepoAccountRole::Custodian` ачааллыг ялгаруулдаг тул асран хамгаалагчид бараа материалыг захиалж, нэгтгэх боломжтой.
* Үс тайралтыг 10000бит/с (100%) хүртэл хавчуулж, хязгаарын давтамж нь бүхэл секунд байна. Хангах
  ажиллах үеийн хүлээлттэй нийцүүлэхийн тулд тэдгээр каноник нэгж дэх засаглалын параметрүүдийг.
* Цагийн тэмдэг нь үргэлж unix миллисекунд байдаг. Бүх туслахууд Norito руу өөрчлөгдөөгүй шилжүүлнэ
  Ачаалал ихтэй тул үе тэнгийнхэн ижил хуваарийг гаргадаг.
* Санаачлах болон задлах заавар нь ижил гэрээний тодорхойлогчийг дахин ашигладаг. Ажиллах цаг нь татгалздаг
  Давхардсан үнэмлэх, үл мэдэгдэх гэрээг задлах; CLI/SDK туслахууд эдгээр алдааг эрт илрүүлдэг.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` каноник хэмнэлийг буцаана. Үргэлж
  Хуучирсан хуваарийг дахин тоглуулахгүйн тулд буцаан дуудлагыг эхлүүлэхийн өмнө энэ агшин зуурын зургийг үзнэ үү.