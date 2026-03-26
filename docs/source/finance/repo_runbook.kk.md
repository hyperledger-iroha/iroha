---
lang: kk
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T16:26:46.568155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Репо есеп айырысу кітабы

Бұл нұсқаулық Iroha ішіндегі репо және кері репо келісімдері үшін детерминирленген ағынды құжаттайды.
Ол CLI оркестрін, SDK көмекшілерін және күтілетін басқару түймелерін қамтиды, осылайша операторлар
шикі Norito пайдалы жүктемелерін жазбай келісімдерді бастаңыз, маржалаңыз және жойыңыз. Басқару үшін
тексеру тізімдерін, дәлелдемелерді алу және алаяқтық/қайтару процедураларын қараңыз
[`repo_ops.md`](./repo_ops.md), ол F1 жол картасы тармағын қанағаттандырады.

## CLI командалары

`iroha app repo` пәрмені репо-арнайы көмекшілерді топтайды:

```bash
# Stage an initiation instruction without submitting
iroha --config client.toml --output \
  repo initiate \
  --agreement-id daily_repo \
  --initiator soraカタカナ... \
  --counterparty soraカタカナ... \
  --custodian soraカタカナ... \
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
  --initiator soraカタカナ... \
  --counterparty soraカタカナ... \
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

* `repo initiate` және `repo unwind` `--input/--output` құрметтейді, сондықтан жасалған `InstructionBox`
  пайдалы жүктемелерді басқа CLI ағындарына жіберуге немесе дереу жіберуге болады.
* Кепілді үш жақты кастодианға бағыттау үшін `--custodian <account>` өтіңіз. Өткізіп тастаған кезде,
  контрагент кепілді тікелей алады (екі жақты репо).
* `repo margin` бухгалтерлік кітапты `FindRepoAgreements` арқылы сұрайды және келесі күтілетін маржаны хабарлайды
  уақыт белгісін (миллисекундпен) және маржаның кері шақыруының қазіргі уақытта қажет екенін көрсетеді.
* `repo margin-call` `RepoMarginCallIsi` нұсқаулығын қосады, маржаны бақылау нүктесін және
  барлық қатысушылар үшін оқиғаларды шығаратын. Қоңыраулар каденция аяқталмаған болса немесе қабылданбайды
  нұсқауды қатысушы емес тұлға береді.

## Python SDK көмекшілері

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

* Екі көмекші де PyO3 байланыстарын шақырмас бұрын сандық шамалар мен метадеректер өрістерін қалыпқа келтіреді.
* `RepoAgreementRecord` жұмыс уақытының кестесін есептеуді көрсетеді, сондықтан бухгалтерлік есептен тыс автоматтандыру мүмкін
  каденсті қолмен қайта есептеместен кері қоңыраулардың қашан болатынын анықтаңыз.

## DvP / PvP есеп айырысулары

`iroha app settlement` пәрмені жеткізу-төлемге қарсы және төлем-төлемге қарсы нұсқаулардың кезеңдерін бөледі:

```bash
# Delivery leg first, then payment
iroha --config client.toml --output \
  settlement dvp \
  --settlement-id trade_dvp \
  --delivery-asset 4fEiy2n5VMFVfi6BzDJge519zAzg \
  --delivery-quantity 10 \
  --delivery-from soraカタカナ... \
  --delivery-to soraカタカナ... \
  --delivery-instrument-id US0378331005 \
  --payment-asset 7EAD8EFYUx1aVKZPUU1fyKvr8dF1 \
  --payment-quantity 1000 \
  --payment-from soraカタカナ... \
  --payment-to soraカタカナ... \
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
  --primary-from soraカタカナ... \
  --primary-to soraカタカナ... \
  --counter-asset 5tPkFK6s2zUcd1qUHyTmY7fDVa2n \
  --counter-quantity 460 \
  --counter-from soraカタカナ... \
  --counter-to soraカタカナ... \
  --iso-xml-out trade_pvp.xml
```

* Аяқ шамалары интегралдық немесе ондық мәндерді қабылдайды және актив дәлдігіне қарсы тексеріледі.
* `--atomicity` `all-or-nothing`, `commit-first-leg` немесе `commit-second-leg` қабылдайды. Осы режимдерді пайдаланыңыз
  Егер келесі өңдеу сәтсіз болса, қай аяқтың орындалатынын білдіру үшін `--order` арқылы (`commit-first-leg`)
  бірінші аяқты қолданады; `commit-second-leg` екіншісін сақтайды).
* CLI шақырулары бүгін бос нұсқаулық метадеректерін шығарады; есеп айырысу деңгейінде Python көмекшілерін пайдаланыңыз
  метадеректерді тіркеу қажет.
* ISO 20022 өрісін салыстыру үшін [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) қараңыз.
  осы нұсқауларға қолдау көрсетеді (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* CLI Norito қатарында канондық XML алдын ала қарауын шығару үшін `--iso-xml-out <path>` өтіңіз.
  нұсқау; файл жоғарыдағы салыстыруды орындайды (DvP үшін `sese.023`, PvP` үшін `sese.025`). жұптаңыз
  жалаушасы `--iso-reference-crosswalk <path>` бар, сондықтан CLI `--delivery-instrument-id` сәйкестігін тексереді.
  бірдей сурет Torii орындау уақытын қабылдау кезінде пайдаланады.

Python көмекшілері CLI бетін көрсетеді:

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

## Детерминизм және басқарудан күтулер

Репо нұсқаулары тек Norito кодталған сандық түрлерге және ортақ
`RepoGovernance::with_defaults` логикасы. Келесі инварианттарды есте сақтаңыз:* Мөлшер детерминирленген `NumericSpec` мәндерімен серияланады: қолма-қол ақшаны пайдалану
  `fractional(2)` (екі ондық таңба), кепіл аяқтары `integer()` пайдаланады. Жібермеңіз
  мәндер дәлдігі жоғарырақ — орындалу уақытының қорғаушылары оларды қабылдамайды және әріптестер алшақтайды.
* Үш жақты реполар `RepoAgreement` ішіндегі кастодиан шотының идентификаторын сақтайды. Өмірлік цикл және маржа оқиғалары
  `RepoAccountRole::Custodian` пайдалы жүктемесін шығарыңыз, осылайша кастодиандар жазылуы және түгендеуді салыстыра алады.
* Шаш қиюлары 10000бит/с (100%) дейін қысылады және шет жиіліктері тұтас секундтар. қамтамасыз ету
  орындау уақытының күтулеріне сәйкес болу үшін сол канондық бірліктердегі басқару параметрлері.
* Уақыт белгілері әрқашан unix миллисекунд болып табылады. Барлық көмекшілер оларды өзгеріссіз Norito жібереді
  пайдалы жүктеме, сондықтан әріптестер бірдей кестелерді шығарады.
* Бастау және босату нұсқаулары бірдей келісім идентификаторын қайта пайдаланады. Орындау уақыты қабылданбайды
  қайталанатын идентификаторлар және белгісіз келісімдерді ашу; CLI/SDK көмекшілері бұл қателерді ертерек анықтайды.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` канондық каденцияны қайтарады. Әрқашан
  ескірген кестелерді қайта ойнатпау үшін кері қоңырауларды іске қоспас бұрын осы суретті қараңыз.