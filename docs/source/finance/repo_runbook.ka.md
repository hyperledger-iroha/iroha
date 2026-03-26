---
lang: ka
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

ეს სახელმძღვანელო ადასტურებს Iroha-ში რეპოსა და უკუ-რეპოს ხელშეკრულებების დეტერმინისტულ ნაკადს.
ის მოიცავს CLI ორკესტრაციას, SDK დამხმარეებს და მოსალოდნელ მართვის ღილაკებს, რათა ოპერატორებმა შეძლონ
შექმენით, შეადგინეთ და გააუქმეთ შეთანხმებები დაუმუშავებელი Norito დატვირთვის გარეშე. მმართველობისთვის
საკონტროლო სიები, მტკიცებულებების აღება და თაღლითობის/დაბრუნების პროცედურები იხ
[`repo_ops.md`](./repo_ops.md), რომელიც აკმაყოფილებს საგზაო რუკის პუნქტს F1.

## CLI ბრძანებები

`iroha app repo` ბრძანება აჯგუფებს რეპო-სპეციფიკურ დამხმარეებს:

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

* `repo initiate` და `repo unwind` პატივს სცემენ `--input/--output`-ს, ამიტომ გენერირებული `InstructionBox`
  ტვირთამწეობა შეიძლება მილებით გადაიტანოს სხვა CLI ნაკადებში ან დაუყოვნებლივ გაგზავნოს.
* გაიარეთ `--custodian <account>` გირაოს გასაგზავნად სამ მხარის მეურვესთან. როდესაც გამოტოვებულია,
  კონტრაგენტი იღებს გირავნობას პირდაპირ (ორმხრივი რეპო).
* `repo margin` ითხოვს წიგნს `FindRepoAgreements`-ის მეშვეობით და აცნობებს შემდეგ მოსალოდნელ ზღვარს
  დროის ანაბეჭდი (მილიწამებში) და ასევე არის თუ არა ზღვრული გამოძახება გაკეთებული.
* `repo margin-call` ანიჭებს `RepoMarginCallIsi` ინსტრუქციას, ჩაწერს ზღვრის საკონტროლო პუნქტს და
  ავრცელებს ღონისძიებებს ყველა მონაწილისთვის. ზარები უარყოფილია, თუ კადენცია არ არის გასული ან თუ
  ინსტრუქცია წარდგენილია არამონაწილის მიერ.

## Python SDK დამხმარეები

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

* ორივე დამხმარე ახდენს რიცხვითი რაოდენობების და მეტამონაცემების ველების ნორმალიზებას PyO3 აკინძების გამოძახებამდე.
* `RepoAgreementRecord` ასახავს მუშაობის დროის განრიგის გამოთვლას, ასე რომ, off-ledger ავტომატიზაციას შეუძლია
  დაადგინეთ, როდის უნდა მოხდეს გამოძახება ხელით კადენციის ხელახალი გამოთვლის გარეშე.

## DvP / PvP დასახლებები

`iroha app settlement` ბრძანება ეტაპებს მიწოდების-გადახდის და გადახდის-გადახდის ინსტრუქციებს:

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

* ფეხის რაოდენობა იღებს ინტეგრალურ ან ათობითი მნიშვნელობებს და დამოწმებულია აქტივის სიზუსტით.
* `--atomicity` იღებს `all-or-nothing`, `commit-first-leg`, ან `commit-second-leg`. გამოიყენეთ ეს რეჟიმები
  `--order`-ით, რათა გამოვხატოთ რომელი ფეხი დარჩება ჩადენილი, თუ შემდგომი დამუშავება ვერ მოხერხდება (`commit-first-leg`
  ინარჩუნებს პირველ ფეხს დაყენებულს; `commit-second-leg` ინარჩუნებს მეორეს).
* CLI გამოძახებები ასხივებს ცარიელ ინსტრუქციის მეტამონაცემებს დღეს; გამოიყენეთ პითონის დამხმარეები დასახლების დონეზე
  მეტამონაცემები უნდა დაერთოს.
* იხილეთ [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) ISO 20022 ველის რუკისთვის, რომელიც
  მხარს უჭერს ამ ინსტრუქციებს (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* გაიარეთ `--iso-xml-out <path>`, რათა CLI გამოუშვას კანონიკური XML გადახედვა Norito-თან ერთად
  ინსტრუქცია; ფაილი მიჰყვება ზემოთ მოცემულ რუკებს (`sese.023` DvP-სთვის, `sese.025` PvP-სთვის). დააწყვილეთ
  მონიშნეთ `--iso-reference-crosswalk <path>`, ასე რომ CLI ამოწმებს `--delivery-instrument-id`-ს
  იგივე სნეპშოტი Torii იყენებს გაშვების დროს.

პითონის დამხმარეები ასახავს CLI ზედაპირს:

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

## დეტერმინიზმი და მმართველობის მოლოდინი

რეპოს ინსტრუქციები ეყრდნობა ექსკლუზიურად Norito-ში დაშიფრულ ციფრულ ტიპებს და გაზიარებულს
`RepoGovernance::with_defaults` ლოგიკა. გაითვალისწინეთ შემდეგი უცვლელები:* რაოდენობები სერიალირებულია დეტერმინისტული `NumericSpec` მნიშვნელობებით: ნაღდი ფულის გამოყენება
  `fractional(2)` (ორი ათობითი ადგილი), გირაოს ფეხები გამოიყენება `integer()`. არ წარადგინო
  ფასეულობები უფრო დიდი სიზუსტით - გაშვების დროს მცველები უარს იტყვიან მათზე და თანატოლები განსხვავდებიან.
* სამმხრივი რეპოები შენარჩუნებულია მეურვის ანგარიშის ID-ში `RepoAgreement`-ში. სასიცოცხლო ციკლი და მარჟის მოვლენები
  გამოუშვით `RepoAccountRole::Custodian` ტვირთამწეობა, რათა მეურვეებმა შეძლონ მარაგის გამოწერა და შეჯერება.
* თმის შეჭრა დამაგრებულია 10000 bps (100%) და ზღვრული სიხშირე არის მთელი წამი. უზრუნველყოს
  მმართველობის პარამეტრები ამ კანონიკურ ერთეულებში, რათა დარჩეს გაშვების მოლოდინებთან.
* დროის შტამპები ყოველთვის უნიქსი მილიწამია. ყველა დამხმარე გადაგზავნის მათ უცვლელად Norito-ზე
  ტვირთამწეობა, რათა თანატოლებმა გამოიტანონ იდენტური გრაფიკები.
* დაწყების და განტვირთვის ინსტრუქციები ხელახლა გამოიყენეთ იგივე შეთანხმების იდენტიფიკატორი. გაშვების დრო უარყოფს
  პირადობის მოწმობების დუბლიკატი და ამოღება უცნობი ხელშეკრულებებისთვის; CLI/SDK-ის დამხმარეები ამ შეცდომებს ადრეულად ავლენენ.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` დააბრუნებს კანონიკურ კადენციას. ყოველთვის
  გაეცანით ამ კადრს, სანამ გამოძახებ გამოძახებას, რათა თავიდან აიცილოთ მოძველებული გრაფიკების ხელახალი თამაში.