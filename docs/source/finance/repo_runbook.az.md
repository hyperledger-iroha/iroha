---
lang: az
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T16:26:46.568155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Repo Hesablaşma Runbook

Bu təlimat Iroha-də repo və əks-repo müqavilələri üçün deterministik axını sənədləşdirir.
O, CLI orkestrasiyası, SDK köməkçiləri və gözlənilən idarəetmə düymələrini əhatə edir ki, operatorlar
xam Norito faydalı yükləri yazmadan müqavilələri başlatın, marja edin və açın. İdarəçilik üçün
yoxlama siyahıları, sübutların tutulması və fırıldaqçılıq/geri qaytarma prosedurlarına baxın
[`repo_ops.md`](./repo_ops.md), F1 yol xəritəsi bəndini təmin edir.

## CLI əmrləri

`iroha app repo` əmri repo-xüsusi köməkçiləri qruplaşdırır:

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

* `repo initiate` və `repo unwind` `--input/--output`-ə hörmət edir, beləliklə yaradılan `InstructionBox`
  faydalı yüklər digər CLI axınlarına ötürülə və ya dərhal təqdim edilə bilər.
* Girovu üçtərəfli qəyyuma yönləndirmək üçün `--custodian <account>` keçin. Buraxıldıqda,
  qarşı tərəf girovu birbaşa alır (ikitərəfli repo).
* `repo margin` kitabı `FindRepoAgreements` vasitəsilə sorğulayır və növbəti gözlənilən marjanı bildirir
  vaxt möhürü (millisaniyələrlə) ilə yanaşı, hazırda marja geri çağırışının vaxtı olub-olmaması.
* `repo margin-call`, marja yoxlama nöqtəsini qeyd edərək, `RepoMarginCallIsi` təlimatını əlavə edir və
  bütün iştirakçılar üçün hadisələr yayan. Zənglər kadans bitməmiş və ya bitməmişsə, rədd edilir
  göstərişi iştirakçı olmayan şəxs təqdim edir.

## Python SDK köməkçiləri

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

* Hər iki köməkçi PyO3 bağlamalarını işə salmazdan əvvəl rəqəmli kəmiyyətləri və metadata sahələrini normallaşdırır.
* `RepoAgreementRecord` iş vaxtı cədvəlinin hesablanmasını əks etdirir, beləliklə, kitabdan kənar avtomatlaşdırma
  kadansı əl ilə yenidən hesablamadan geri çağırışların nə vaxt olacağını müəyyənləşdirin.

## DvP / PvP hesablaşmaları

`iroha app settlement` əmri çatdırılma-ödəniş və ödəniş-ödəniş təlimatları mərhələlərini əhatə edir:

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

* Ayaq kəmiyyətləri inteqral və ya onluq dəyərləri qəbul edir və aktivin dəqiqliyinə uyğun olaraq təsdiqlənir.
* `--atomicity` `all-or-nothing`, `commit-first-leg` və ya `commit-second-leg` qəbul edir. Bu rejimlərdən istifadə edin
  sonrakı emal uğursuz olarsa, hansı ayağın sadiq qalacağını ifadə etmək üçün `--order` ilə (`commit-first-leg`
  ilk ayağı tətbiq edir; `commit-second-leg` ikincini saxlayır).
* CLI çağırışları bu gün boş təlimat metadatasını yayır; məskunlaşma səviyyəsində olduqda Python köməkçilərindən istifadə edin
  metadata əlavə edilməlidir.
* ISO 20022 sahə xəritəsi üçün [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) baxın
  bu təlimatları dəstəkləyir (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* CLI-nin Norito ilə yanaşı kanonik XML önizləməsini yayması üçün `--iso-xml-out <path>` keçin
  təlimat; fayl yuxarıdakı xəritələşdirməni izləyir (DvP üçün `sese.023`, PvP` üçün `sese.025`). cütləşdirin
  `--iso-reference-crosswalk <path>` ilə işarələyin, beləliklə CLI `--delivery-instrument-id`-i təsdiqləyir
  eyni snapshot Torii iş vaxtı qəbulu zamanı istifadə edir.

Python köməkçiləri CLI səthini əks etdirir:

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

## Determinizm və İdarəetmə Gözləntiləri

Repo təlimatları yalnız Norito kodlu rəqəmsal tiplərə və paylaşılan
`RepoGovernance::with_defaults` məntiqi. Aşağıdakı invariantları yadda saxlayın:* Miqdarlar deterministik `NumericSpec` dəyərləri ilə seriallaşdırılıb: pul ayaqları istifadə
  `fractional(2)` (iki onluq yer), girov ayaqları `integer()` istifadə edir. Təqdim etməyin
  daha dəqiqliklə dəyərlər - iş vaxtı mühafizəçiləri onları rədd edəcək və həmyaşıdları bir-birindən ayrılacaq.
* Üçtərəfli repolar `RepoAgreement`-də mühafizəçi hesab identifikatorunu saxlayır. Həyat dövrü və marja hadisələri
  `RepoAccountRole::Custodian` faydalı yükü buraxın ki, qəyyumlar abunə oluna və inventarla barışa bilsinlər.
* Saç kəsimləri 10000bps (100%) ilə sıxılır və kənar tezliklər tam saniyədir. təmin etmək
  icra vaxtı gözləntilərinə uyğun qalmaq üçün həmin kanonik vahidlərdə idarəetmə parametrləri.
* Vaxt damğaları həmişə unix millisaniyədir. Bütün köməkçilər onları dəyişmədən Norito-ə yönləndirirlər
  həmyaşıdların eyni cədvəllər əldə etməsi üçün faydalı yük.
* Başlama və açma təlimatları eyni razılaşma identifikatorundan təkrar istifadə edir. İcra müddəti rədd edir
  dublikat şəxsiyyət vəsiqələri və naməlum müqavilələr üçün açılma; CLI/SDK köməkçiləri bu səhvləri erkən aşkarlayır.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` kanonik kadansı qaytarır. Həmişə
  köhnə cədvəlləri təkrarlamamaq üçün geri çağırışları işə salmazdan əvvəl bu snapshotla məsləhətləşin.