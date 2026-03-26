---
lang: uz
direction: ltr
source: docs/source/finance/repo_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d5dd8e1b666be34bb9101898d355fe5e3c6efc32500c238c72a6ef9228c157f0
source_last_modified: "2026-01-22T16:26:46.568155+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Repo hisob-kitoblar kitobi

Ushbu qo'llanma Iroha da repo va teskari repo shartnomalari uchun deterministik oqimni hujjatlashtiradi.
U CLI orkestratsiyasi, SDK yordamchilari va kutilayotgan boshqaruv tugmalarini qamrab oladi, shuning uchun operatorlar
xom Norito foydali yuklarni yozmasdan kelishuvlarni boshlash, cheklash va bekor qilish. Boshqaruv uchun
nazorat ro'yxatlari, dalillarni qo'lga olish va firibgarlik/orqaga qaytarish tartib-qoidalarini ko'ring
[`repo_ops.md`](./repo_ops.md), bu F1 yoʻl xaritasi bandiga javob beradi.

## CLI buyruqlari

`iroha app repo` buyrug'i repo-maxsus yordamchilarni guruhlaydi:

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

* `repo initiate` va `repo unwind` `--input/--output` ni hurmat qiladi, shuning uchun yaratilgan `InstructionBox`
  foydali yuklarni boshqa CLI oqimlariga yuborish yoki darhol yuborish mumkin.
* Garovni uch tomonlama qo'riqchiga yo'naltirish uchun `--custodian <account>` orqali o'ting. O'tkazib yuborilganda,
  kontragent garovni bevosita oladi (ikki tomonlama repo).
* `repo margin` daftarni `FindRepoAgreements` orqali so'raydi va keyingi kutilgan marja haqida xabar beradi
  vaqt tamg'asi (millisekundlarda) bilan bir qatorda chegarani qayta qo'ng'iroq qilish kerakmi yoki yo'qmi.
* `repo margin-call` `RepoMarginCallIsi` ko'rsatmasini qo'shib, chekni tekshirish nuqtasini va
  barcha ishtirokchilar uchun hodisalarni tarqatish. Agar kadans o'tmagan bo'lsa yoki qo'ng'iroqlar rad etiladi
  ko'rsatma ishtirok etmagan shaxs tomonidan taqdim etiladi.

## Python SDK yordamchilari

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

* Ikkala yordamchi ham PyO3 ulanishlarini chaqirishdan oldin raqamli miqdorlar va metadata maydonlarini normallashtiradi.
* `RepoAgreementRecord` ish vaqti jadvalini hisoblashni aks ettiradi, shuning uchun hisobdan tashqari avtomatlashtirish mumkin
  kadansni qo'lda qayta hisoblamasdan, qayta qo'ng'iroqlar qachon kelishini aniqlang.

## DvP / PvP hisob-kitoblari

`iroha app settlement` buyrug'i yetkazib berish-to'lovga qarshi va to'lov-to'lovga qarshi ko'rsatmalar bosqichlarini ajratadi:

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

* Oyoq miqdorlari integral yoki kasr qiymatlarini qabul qiladi va aktivning aniqligi bilan tasdiqlanadi.
* `--atomicity` `all-or-nothing`, `commit-first-leg` yoki `commit-second-leg` ni qabul qiladi. Ushbu rejimlardan foydalaning
  `--order` bilan, agar keyingi ishlov berish muvaffaqiyatsiz bo'lsa, qaysi oyog'i sodiq qolishini bildirish uchun (`commit-first-leg`)
  qo'llaniladigan birinchi oyoqni ushlab turadi; `commit-second-leg` ikkinchisini saqlab qoladi).
* CLI chaqiruvlari bugungi kunda bo'sh ko'rsatmalar metama'lumotlarini chiqaradi; hisob-kitob darajasida Python yordamchilaridan foydalaning
  metama'lumotlar biriktirilishi kerak.
* ISO 20022 maydon xaritasi uchun [`settlement_iso_mapping.md`](./settlement_iso_mapping.md) ga qarang.
  ushbu ko'rsatmalarni qo'llab-quvvatlaydi (`sese.023`, `sese.025`, `colr.007`, `pacs.009`, `camt.054`).
* CLI Norito bilan birga kanonik XML ko'rinishini chiqarishi uchun `--iso-xml-out <path>` dan o'ting
  ko'rsatma; fayl yuqoridagi xaritaga amal qiladi (DvP uchun `sese.023`, PvP` uchun `sese.025`). ni juftlashtiring
  `--iso-reference-crosswalk <path>` bayrog'i bilan belgilang, shuning uchun CLI `--delivery-instrument-id` ga qarshi tekshiradi.
  xuddi shu surat Torii ish vaqti qabul qilish vaqtida foydalanadi.

Python yordamchilari CLI sirtini aks ettiradi:

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

## Determinizm va boshqaruv umidlari

Repo ko'rsatmalari faqat Norito kodlangan raqamli turlarga va umumiy
`RepoGovernance::with_defaults` mantiqiy. Quyidagi invariantlarni yodda tuting:* Miqdorlar deterministik `NumericSpec` qiymatlari bilan ketma-ketlashtiriladi: naqd puldan foydalanish
  `fractional(2)` (ikki kasr), garov oyoqlari `integer()` dan foydalanadi. Taqdim qilmang
  ko'proq aniqlik bilan qiymatlar - ish vaqti qo'riqchilari ularni rad etadi va tengdoshlar ajralib chiqadi.
* Uch tomonli repolar `RepoAgreement` da kastodian hisobi identifikatorini saqlab qoladi. Hayotiy tsikl va chegara hodisalari
  `RepoAccountRole::Custodian` foydali yukini chiqaring, shunda vasiylar obuna bo'lishi va inventarni yarashtirishi mumkin.
* Sartaroshlar 10000bps (100%) ga mahkamlanadi va chekka chastotalar butun soniyalardir. Ta'minlash
  o'sha kanonik birliklardagi boshqaruv parametrlari ish vaqti kutilganiga mos keladi.
* Vaqt belgilari har doim unix millisekundlardan iborat. Barcha yordamchilar ularni o'zgarmagan holda Norito ga yuboradilar
  Tengdoshlar bir xil jadvallarni olishlari uchun foydali yuk.
* Boshlash va ochish bo'yicha ko'rsatmalar bir xil kelishuv identifikatorini qayta ishlatadi. Ishlash vaqti rad etadi
  dublikat identifikatorlari va noma'lum kelishuvlarni ochish; CLI/SDK yordamchilari bu xatolarni erta aniqlashadi.
* `repo margin`/`RepoAgreementRecord::next_margin_check_after` kanonik kadansni qaytaradi. Har doim
  Eski jadvallarni takrorlamaslik uchun qayta qo'ng'iroqlarni ishga tushirishdan oldin ushbu suratga qarang.