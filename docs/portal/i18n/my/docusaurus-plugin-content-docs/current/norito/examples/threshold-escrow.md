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

```text
// Threshold escrow sample for a single payer and an exact funding target.
// The payer is bound to authority() when the escrow is opened.
seiyaku ThresholdEscrow {
  meta { abi_version: 1; }

  state AccountId payer_account;
  state AccountId recipient_account;
  state AccountId escrow_account_id;
  state AssetDefinitionId escrow_asset_definition;
  state int target_amount_value;
  state int funded_amount_value;
  state bool is_open;
  state bool is_released;
  state bool is_refunded;

  fn assert_unopened() {
    assert(!is_open, "escrow already open");
    assert(!is_released, "escrow already released");
    assert(!is_refunded, "escrow already refunded");
  }

  fn assert_open() {
    assert(is_open, "escrow is not open");
    assert(!is_released, "escrow already released");
    assert(!is_refunded, "escrow already refunded");
  }

  fn assert_payer() {
    assert(authority() == payer_account, "only the payer may call this entrypoint");
  }

  kotoage fn main() {}

  // NOTE:
  // This sample uses permission(Admin) because it releases and refunds funds
  // from the configured escrow account.
  #[access(read="*", write="*")]
  kotoage fn open_escrow(recipient: AccountId,
                         escrow_account: AccountId,
                         asset_definition: AssetDefinitionId,
                         target_amount: int) permission(Admin) {
    assert_unopened();
    assert(target_amount > 0, "target_amount must be positive");

    payer_account = authority();
    recipient_account = recipient;
    escrow_account_id = escrow_account;
    escrow_asset_definition = asset_definition;
    target_amount_value = target_amount;
    funded_amount_value = 0;
    is_open = true;
    is_released = false;
    is_refunded = false;
  }

  #[access(read="*", write="*")]
  kotoage fn deposit(amount: int) permission(Admin) {
    assert_open();
    assert_payer();
    assert(amount > 0, "amount must be positive");

    let next_funded = funded_amount_value + amount;
    assert(next_funded <= target_amount_value, "deposit exceeds target_amount");

    transfer_asset(payer_account, escrow_account_id, escrow_asset_definition, amount);
    funded_amount_value = next_funded;
  }

  #[access(read="*", write="*")]
  kotoage fn release_if_ready() permission(Admin) {
    assert_open();
    assert(funded_amount_value == target_amount_value, "escrow is not fully funded");

    transfer_asset(
      escrow_account_id,
      recipient_account,
      escrow_asset_definition,
      funded_amount_value
    );
    is_open = false;
    is_released = true;
  }

  #[access(read="*", write="*")]
  kotoage fn refund() permission(Admin) {
    assert_open();
    assert_payer();

    let funded = funded_amount_value;
    if (funded > 0) {
      transfer_asset(
        escrow_account_id,
        payer_account,
        escrow_asset_definition,
        funded
      );
    }
    is_open = false;
    is_refunded = true;
  }
}
```