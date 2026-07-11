<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
direction: ltr
source: docs/portal/docs/norito/examples/threshold-escrow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 54b6d543cff8df6e8fd50632cfed6265770edc33855f06912be603457c5b517e
source_last_modified: "2026-04-02T18:31:54.074495+00:00"
translation_last_reviewed: 2026-04-08
translator: machine-google-reviewed
---

---
slug: /norito/examples/threshold-escrow
title: Эскроугийн босго
description: Товлосон дүнгийн хэмжээгээр цэнэглэхийг хүлээн зөвшөөрч, дараа нь мөнгийг чөлөөлөх эсвэл буцаан олгох нэг төлбөр төлөгчийн эскроу.
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

Товлосон дүнгийн хэмжээгээр цэнэглэлтийг хүлээн авч, дараа нь мөнгийг чөлөөлөх эсвэл буцаан олгох нэг төлбөр төлөгчийн эскроу.

## Бүртгэлийн дэвтэр

- Эскроу данс болон хөрөнгийн тоон тодорхойлолтыг урьдчилан үүсгэж, дараа нь гэрээний дуудлагыг илгээх төлбөр төлөгчийн дансыг санхүүжүүлнэ. Дээж нь `open_escrow` үед төлбөр төлөгчийг `authority()`-тэй автоматаар холбодог.
- `open_escrow(recipient, escrow_account, asset_definition, target_amount)` руу нэг удаа залгаж төлбөр төлөгч, хүлээн авагч, эскроу данс, хөрөнгийн тодорхойлолт, тодорхой зорилт, нээлттэй/суллагдсан/буцаасан дарцагуудыг бат бөх гэрээний төлөвт бүртгэнэ.
- Нэг төлбөр төлөгчөөс `funded_amount_value == target_amount_value` хүртэл `deposit(amount)` руу залгах; хадгаламж эерэг хэвээр байх ёстой бөгөөд эскроуг хэтрүүлэн санхүүжүүлэх аливаа цэнэглэлтээс татгалзана.
- Зорилтот хэмжээнд хүрсэний дараа хадгалсан мөнгийг хүлээн авагч руу шилжүүлэхийн тулд `release_if_ready()` руу залгах, эскроу нээлттэй хэвээр байхад `refund()` руу залгаж санхүүжүүлсэн мөнгийг төлбөр төлөгчид буцааж өгнө.
- Үлдэгдлийг `FindAssetById` / `iroha ledger asset list all --verbose`, `GET /v1/contracts/state?paths=payer_account,recipient_account,escrow_account_id,escrow_asset_definition,target_amount_value,funded_amount_value,is_open,is_released,is_refunded&decode=json` ашиглан гэрээний төлөвийг шалгана уу.

## Холбогдох SDK гарын авлага

- [Зэв SDK хурдан эхлүүлэх](/sdks/rust)
- [Python SDK хурдан эхлүүлэх](/sdks/python)
- [JavaScript SDK хурдан эхлүүлэх](/sdks/javascript)

[Kotodama эх сурвалжийг татаж авах](/norito-snippets/threshold-escrow.ko)

```kotodama
// Threshold escrow sample for a single payer and an exact funding target.
// The payer is bound to context::authority() when the escrow is opened.
seiyaku ThresholdEscrow {
    error enum EscrowError {
        AlreadyOpen = 1, AlreadyReleased = 2, AlreadyRefunded = 3, NotOpen = 4, UnauthorizedPayer = 5, NonPositiveTarget = 6, NonPositiveAmount = 7, TargetExceeded = 8, NotFullyFunded = 9,
    }

    const recipient_account_literal: string = "sorauﾛ1PｽNgｿﾘ9ﾏﾕ2ﾕ9ﾄZﾀﾃﾌWwNｸｾヰﾄﾂT3WｺTxｶｵﾎKﾓﾛmｷ4Y6PLN";
    const escrow_account_literal: string = "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76";
    const escrow_asset_definition_literal: string = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    state payer_account: AccountId;
    state recipient_account: AccountId;
    state escrow_account_id: AccountId;
    state escrow_asset_definition: AssetDefinitionId;
    state target_amount_value: Amount;
    state funded_amount_value: Amount;
    state is_open: bool;
    state is_released: bool;
    state is_refunded: bool;
    hajimari() {
        let zero = Amount::from_i64(0);
        payer_account = context::authority();
        recipient_account = AccountId::parse(recipient_account_literal);
        escrow_account_id = AccountId::parse(escrow_account_literal);
        escrow_asset_definition = AssetDefinitionId::parse(escrow_asset_definition_literal);
        target_amount_value = zero;
        funded_amount_value = zero;
        is_open = false;
        is_released = false;
        is_refunded = false;
    }

    fn assert_unopened() {
        require(!is_open, EscrowError::AlreadyOpen);
        require(!is_released, EscrowError::AlreadyReleased);
        require(!is_refunded, EscrowError::AlreadyRefunded);
    }

    fn assert_open() {
        require(is_open, EscrowError::NotOpen);
        require(!is_released, EscrowError::AlreadyReleased);
        require(!is_refunded, EscrowError::AlreadyRefunded);
    }

    fn assert_payer() {
        require(context::authority() == payer_account, EscrowError::UnauthorizedPayer);
    }

    // NOTE:
    // This sample uses authorize("Admin") because it releases and refunds funds
    // from the configured escrow account. The recipient, escrow account, and
    // asset definition are fixed literals so the compiler can emit a complete
    // first-release access set without manual annotations.
    kotoage fn open_escrow(target_amount: Amount) authorize("Admin") {
        assert_unopened();
        let zero = Amount::from_i64(0);
        require(target_amount > zero, EscrowError::NonPositiveTarget);
        payer_account = context::authority();
        recipient_account = AccountId::parse(recipient_account_literal);
        escrow_account_id = AccountId::parse(escrow_account_literal);
        escrow_asset_definition = AssetDefinitionId::parse(escrow_asset_definition_literal);
        target_amount_value = target_amount;
        funded_amount_value = zero;
        is_open = true;
        is_released = false;
        is_refunded = false;
    }

    kotoage fn deposit(amount: Amount) authorize("Admin") {
        assert_open();
        assert_payer();
        require(amount > Amount::from_i64(0), EscrowError::NonPositiveAmount);
        let next_funded = funded_amount_value + amount;
        require(next_funded <= target_amount_value, EscrowError::TargetExceeded);
        ledger::asset::transfer(
            source: context::authority(),
            destination: AccountId::parse(escrow_account_literal),
            asset_definition: AssetDefinitionId::parse(escrow_asset_definition_literal),
            amount: amount,
            dataspace: DataSpaceId::parse("0"),
        );
        funded_amount_value = next_funded;
    }

    kotoage fn release_if_ready() authorize("Admin") {
        assert_open();
        require(funded_amount_value == target_amount_value, EscrowError::NotFullyFunded);
        ledger::asset::transfer(
            source: AccountId::parse(escrow_account_literal),
            destination: AccountId::parse(recipient_account_literal),
            asset_definition: AssetDefinitionId::parse(escrow_asset_definition_literal),
            amount: funded_amount_value,
            dataspace: DataSpaceId::parse("0"),
        );
        is_open = false;
        is_released = true;
    }

    kotoage fn refund() authorize("Admin") {
        assert_open();
        assert_payer();
        let funded = funded_amount_value;
        if (funded > Amount::from_i64(0)) {
            ledger::asset::transfer(
                source: AccountId::parse(escrow_account_literal),
                destination: context::authority(),
                asset_definition: AssetDefinitionId::parse(escrow_asset_definition_literal),
                amount: funded,
                dataspace: DataSpaceId::parse("0"),
            );
        }
        is_open = false;
        is_refunded = true;
    }
}
```
