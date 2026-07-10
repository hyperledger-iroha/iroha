<!-- Auto-generated stub for Dzongkha (dz) translation. Replace this content with the full translation. -->

---
lang: dz
direction: ltr
source: docs/portal/docs/norito/examples/threshold-escrow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
source_hash: 270c9c1079659b7c0f66d20b22a4453620d87bdb2877e66600db4a4b844c7924
source_last_modified: "2026-04-02T18:31:54.074495+00:00"
translation_last_reviewed: 2026-04-02
slug: /norito/examples/threshold-escrow
title: ཐེམ་ཐོ་ཨེས་ཀོརོ།
description: དམིགས་གཏད་ཅན་གྱི་དངུལ་འབོར་ངེས་བདེན་ལུ་ ཁ་སྐོང་བཀལ་མི་ཚུ་ ངོས་ལེན་འབད་མི་ དངུལ་སྤྲོད་མི་རྐྱང་པའི་ བཀག་ཆ་ དེ་ལས་ མ་དངུལ་ཚུ་ བཏོན་གཏང་ནི་དང་ ཡང་ན་ ལོག་སྤྲོདཔ་ཨིན།
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

དམིགས་གཏད་ཅན་གྱི་དངུལ་འབོར་ངེས་བདེན་ལུ་ ཁ་སྐོང་བཀལ་མི་ཚུ་ ངོས་ལེན་འབད་མི་ དངུལ་ཕོགས་སྤྲོད་མི་རྐྱང་པ་གིས་ མ་དངུལ་ཚུ་ བཏོན་གཏང་ནི་དང་ ཡང་ན་ ལོག་སྤྲོདཔ་ཨིན།

## ལེ་ཇར་འགྲུལ་བཞུད་

- ཨེསི་ཀོརོ་རྩིས་ཁྲ་དང་ ཨང་གྲངས་རྒྱུ་དངོས་ངེས་ཚིག་ཚུ་ སྔོན་སྒྲིག་འབད་ཞིནམ་ལས་ གན་རྒྱ་འབོད་བརྡ་ཚུ་ བཙུགས་མི་ དངུལ་སྤྲོད་མི་རྩིས་ཁྲ་ལུ་ མ་དངུལ་བཙུགས། དཔེ་ཚད་འདི་གིས་ དངུལ་སྤྲོད་མི་འདི་ `open_escrow` གི་སྐབས་ལུ་ `authority()` དང་ཅིག་ཁར་ རང་བཞིན་གྱིས་ བསྡམ་བཞགཔ་ཨིན།
- གླ་ཆ་སྤྲོད་མི་དང་ ཐོབ་མི་ བཀག་ཆ་རྩིས་ཁྲ་ རྒྱུ་དངོས་ངེས་ཚིག་ དམིགས་ཚད་ངེས་བདེན་ དེ་ལས་ ཐུབ་ཚད་ཅན་གྱི་གན་རྒྱ་གནས་སྟངས་ནང་ ཁ་ཕྱེ་/བཏོན་བཏང་མི་/ལོག་སྤྲོད་མི་ དར་ཆ་ཚུ་ ཐོ་བཀོད་འབད་ནིའི་དོན་ལུ་ `open_escrow(recipient, escrow_account, asset_definition, target_amount)` ལུ་ ཚར་གཅིག་ཁ་པར་གཏང་།
- དངུལ་སྤྲོད་མི་གཅིག་ལས་ `funded_amount_value == target_amount_value` ཚུན་ཚོད་ `deposit(amount)` ལུ་ཁ་པར་གཏང་། དངུལ་བཙུགས་ཚུ་ ལེགས་ཤོམ་སྦེ་རང་ བཞག་དགོཔ་དང་ མ་དངུལ་མང་དྲགས་སྦེ་ བཙུགས་མི་ཚུ་ ངོས་ལེན་མི་འབད།
- དམིགས་གཏད་གྲུབ་ཚར་བའི་ཤུལ་ལས་ བཀག་ཆ་འབད་མི་མ་དངུལ་ཚུ་ ཐོབ་མི་ལུ་སྤོ་བཤུད་འབད་ནིའི་དོན་ལུ་ `release_if_ready()` ལུ་ཁ་པར་གཏང་ ཡང་ན་ བཀག་ཆ་འབད་མི་འདི་ ད་ལྟོ་ཡང་ ཁ་ཕྱེ་སྟེ་ཡོད་པའི་སྐབས་ `refund()` ལུ་ཁ་པར་གཏང་སྟེ་ མ་དངུལ་སྤྲོད་མི་ལུ་ ལོག་སྤྲོད་དགོ།
- `FindAssetById` / `iroha_cli ledger asset list` དང་ཅིག་ཁར་ ལྷག་ལུས་ཚུ་བརྟག་དཔྱད་འབད་ཞིནམ་ལས་ `GET /v1/contracts/state?paths=payer_account,recipient_account,escrow_account_id,escrow_asset_definition,target_amount_value,funded_amount_value,is_open,is_released,is_refunded&decode=json` དང་ཅིག་ཁར་ གན་རྒྱ་གནས་སྟངས་བརྟག་དཔྱད་འབད།

## འབྲེལ་ཡོད་ཨེསི་ཌི་ཀེ་ལམ་སྟོན།

- [རསཊ་ཨེསི་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས་](/sdks/rust)
- [པའི་ཐོན་ཨེསི་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས](/sdks/python)
- [ཇ་བ་སི་ཀིརིཔ་ཊི་ཨེསི་ཌི་ཀེ་མགྱོགས་འགོ་བཙུགས་](/sdks/javascript)

[Kotodama འབྱུང་ཁུངས་ཕབ་ལེན་བྱོས།](/norito-snippets/threshold-escrow.ko)

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
        ledger::asset::transfer(context::authority(), AccountId::parse(escrow_account_literal), AssetDefinitionId::parse(escrow_asset_definition_literal), amount, DataSpaceId::parse("0"));
        funded_amount_value = next_funded;
    }

    kotoage fn release_if_ready() authorize("Admin") {
        assert_open();
        require(funded_amount_value == target_amount_value, EscrowError::NotFullyFunded);
        ledger::asset::transfer(AccountId::parse(escrow_account_literal), AccountId::parse(recipient_account_literal), AssetDefinitionId::parse(escrow_asset_definition_literal), funded_amount_value, DataSpaceId::parse("0"));
        is_open = false;
        is_released = true;
    }

    kotoage fn refund() authorize("Admin") {
        assert_open();
        assert_payer();
        let funded = funded_amount_value;
        if (funded > Amount::from_i64(0)) {
            ledger::asset::transfer(AccountId::parse(escrow_account_literal), context::authority(), AssetDefinitionId::parse(escrow_asset_definition_literal), funded, DataSpaceId::parse("0"));
        }
        is_open = false;
        is_refunded = true;
    }
}
```
