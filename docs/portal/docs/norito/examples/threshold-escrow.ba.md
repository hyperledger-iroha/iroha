<!-- Auto-generated stub for Bashkir (ba) translation. Replace this content with the full translation. -->

---
lang: ba
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
title: Порог эскроу
description: Бер түләүсе эскроу, тип ҡабул итә, тултырыу өсөн теүәл маҡсатлы сумма, һуңынан сығара йәки аҡса ҡайтара.
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

Бер түләүсе эскроу, тип ҡабул итә, тултырыу өсөн теүәл маҡсатлы сумма, һуңынан сығара йәки аҡса ҡайтара.

## Баш китабы үткәреү

- Алдан эскроу-иҫәп һәм һанлы активтарҙы билдәләү булдырыу, һуңынан түләүсе иҫәбен финанслау, тип тапшырасаҡ килешеп шылтыратыуҙар. Өлгө автоматик рәүештә был түләүсе менән `open_escrow` ваҡытында `authority()` менән бәйләй.
- Шылтыратыу `open_escrow(recipient, escrow_account, asset_definition, target_amount)` бер тапҡыр яҙып алыу өсөн түләүсе, алыусы, эскроу иҫәбенә, активтарҙы билдәләү, теүәл маҡсат, һәм асыҡ/бушатылған/ҡайтарылған флагтар ныҡлы килешеп дәүләт.
- Шул уҡ түләүсенән `funded_amount_value == target_amount_value` тиклем `deposit(amount)` шылтыратыу; депозиттар ыңғай ҡалырға тейеш һәм теләһә ниндәй тултырыу, тип артыҡ финанслау эскроу кире ҡағыла.
- `release_if_ready()` шылтыратығыҙ, маҡсатҡа ирешкәс, эскроу аҡсаһын алыусыға күсерергә, йәки эскроу асыҡ булғанда `refund()` шылтыратығыҙ, финансланған сумманы түләүсегә кире ҡайтарығыҙ.
- 18НИ00000013Х / 18НИ00000014Х менән баланстарҙы тикшерергә һәм 18НИ00000015Х менән килешелгән хәлде тикшерергә.

## SDK-ға бәйле ҡулланмалар

- [Раст SDK тиҙ башлау] (/sdks/rust)
- [Питон SDK тиҙ башлау] (/sdks/python)
- [Яваскрипт SDK тиҙ башлау] (/sdks/javascript)

[Скачать источник Kotodama] (/norito-snippets/threshold-escrow.ko)

18НФ00000001Х

```kotodama
// Threshold escrow sample for a single payer and an exact funding target.
// The payer is bound to context::authority() when the escrow is opened.
seiyaku ThresholdEscrow {
    error enum EscrowError {
        AlreadyOpen = 1, AlreadyReleased = 2, AlreadyRefunded = 3, NotOpen = 4, UnauthorizedPayer = 5, NonPositiveTarget = 6, NonPositiveAmount = 7, TargetExceeded = 8, NotFullyFunded = 9,
    }

    const string recipient_account_literal = "sorauﾛ1PｽNgｿﾘ9ﾏﾕ2ﾕ9ﾄZﾀﾃﾌWwNｸｾヰﾄﾂT3WｺTxｶｵﾎKﾓﾛmｷ4Y6PLN";
    const string escrow_account_literal = "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76";
    const string escrow_asset_definition_literal = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
    state AccountId payer_account;
    state AccountId recipient_account;
    state AccountId escrow_account_id;
    state AssetDefinitionId escrow_asset_definition;
    state quantity target_amount_value;
    state quantity funded_amount_value;
    state bool is_open;
    state bool is_released;
    state bool is_refunded;
    hajimari() {
        let quantity zero = 0;
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
    kotoage fn open_escrow(quantity target_amount) authorize("Admin") {
        assert_unopened();
        let quantity zero = 0;
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

    kotoage fn deposit(quantity amount) authorize("Admin") {
        assert_open();
        assert_payer();
        require(amount > 0, EscrowError::NonPositiveAmount);
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
        if (funded > 0) {
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
