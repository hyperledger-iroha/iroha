<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/portal/docs/norito/examples/threshold-escrow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
source_hash: 270c9c1079659b7c0f66d20b22a4453620d87bdb2877e66600db4a4b844c7924
source_last_modified: "2026-04-02T18:31:54.074495+00:00"
translation_last_reviewed: 2026-04-02
slug: /norito/examples/threshold-escrow
title: တံခါးပေါက် အာမခံ
description: တိကျသောပစ်မှတ်ပမာဏတစ်ခုသို့ ငွေဖြည့်သွင်းမှုများကို လက်ခံသည့် တစ်ခုတည်းသောငွေပေးချေသူ အာမခံသည် ထို့နောက် ရန်ပုံငွေများကို ထုတ်လွှတ်ခြင်း သို့မဟုတ် ပြန်အမ်းပေးခြင်း။
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

တိကျသောပစ်မှတ်ပမာဏတစ်ခုသို့ ငွေဖြည့်သွင်းမှုများကို လက်ခံသည့် တစ်ခုတည်းသောငွေပေးချေသူ အာမခံသည် ထို့နောက် ရန်ပုံငွေများကို ထုတ်လွှတ်ခြင်း သို့မဟုတ် ပြန်အမ်းပေးခြင်း။

## လယ်ဂျာရှင်းလင်းချက်

- escrow အကောင့်နှင့် ဂဏန်းပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်ကို ကြိုတင်ဖန်တီးပါ၊ ထို့နောက် စာချုပ်ခေါ်ဆိုမှုများကို တင်သွင်းမည့် ငွေပေးချေသူအကောင့်ကို ရန်ပုံငွေထည့်ပါ။ နမူနာသည် `open_escrow` အတွင်း `authority()` နှင့် အလိုအလျောက် စည်းနှောင်ထားသည်။
- ငွေပေးချေသူ၊ လက်ခံသူ၊ အာမခံအကောင့်၊ ပိုင်ဆိုင်မှုသတ်မှတ်ချက်၊ အတိအကျပစ်မှတ်၊ နှင့် တာရှည်ခံစာချုပ်အခြေအနေတွင် ဖွင့်/ထုတ်/ပြန်အမ်းပေးသည့် အလံများကို မှတ်တမ်းတင်ရန် `open_escrow(recipient, escrow_account, asset_definition, target_amount)` ကို တစ်ကြိမ်ခေါ်ဆိုပါ။
- `deposit(amount)` ကို `funded_amount_value == target_amount_value` အထိ တူညီသောငွေပေးသူထံမှ `deposit(amount)` သို့ခေါ်ဆိုပါ။ အပ်ငွေများသည် အပြုသဘောဆောင်နေရမည် ဖြစ်ပြီး escrow ကို ငွေပိုပေးမည့် မည်သည့်ငွေဖြည့်သွင်းမှုကိုမဆို ပယ်ချပါသည်။
- ပစ်မှတ်ပြည့်သွားသည်နှင့် လက်ခံရရှိထားသော ရန်ပုံငွေများကို လက်ခံသူထံ ပြောင်းရွှေ့ရန် `release_if_ready()` သို့ ဖုန်းခေါ်ဆိုပါ သို့မဟုတ် ငွေပေးချေသူထံ ရန်ပုံငွေပမာဏကို ပြန်ပေးရန်အတွက် `refund()` သို့ ဖုန်းခေါ်ဆိုပါ။
- `FindAssetById` / `iroha_cli ledger asset list` ဖြင့် လက်ကျန်များကို စစ်ဆေးပြီး `GET /v1/contracts/state?paths=payer_account,recipient_account,escrow_account_id,escrow_asset_definition,target_amount_value,funded_amount_value,is_open,is_released,is_refunded&decode=json` ဖြင့် စာချုပ်အခြေအနေကို စစ်ဆေးပါ။

## သက်ဆိုင်ရာ SDK လမ်းညွှန်များ

- [Rrust SDK အမြန်စတင်ခြင်း](/sdks/rust)
- [Python SDK အမြန်စတင်ခြင်း](/sdks/python)
- [JavaScript SDK အမြန်စတင်ခြင်း](/sdks/javascript)

[Kotodama အရင်းအမြစ်ကို ဒေါင်းလုဒ်လုပ်ပါ](/norito-snippets/threshold-escrow.ko)

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
