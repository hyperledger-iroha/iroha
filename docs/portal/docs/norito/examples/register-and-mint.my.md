<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e686495c642a08740504c4bb5f88e623c89a896787388b61e4451f550f87af6
source_last_modified: "2026-03-26T13:01:47.376183+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/register-and-mint
title: ဒိုမိန်းနှင့် mint ပိုင်ဆိုင်မှုများကို မှတ်ပုံတင်ပါ။
description: ခွင့်ပြုထားသော ဒိုမိန်းဖန်တီးခြင်း၊ ပိုင်ဆိုင်မှုမှတ်ပုံတင်ခြင်းနှင့် အဆုံးအဖြတ်ပေးခြင်းတို့ကို သရုပ်ပြသည်။
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

ခွင့်ပြုထားသော ဒိုမိန်းဖန်တီးခြင်း၊ ပိုင်ဆိုင်မှုမှတ်ပုံတင်ခြင်းနှင့် အဆုံးအဖြတ်ပေးခြင်းတို့ကို သရုပ်ပြသည်။

## လယ်ဂျာရှင်းလင်းချက်

- ဦးတည်ရာအကောင့် (ဥပမာ၊ Alice အတွက် `<i105-account-id>`) ရှိနေကြောင်း သေချာစေပြီး SDK တစ်ခုစီတွင် အမြန်စတင်ခြင်း အဆင့်ကို ရောင်ပြန်ဟပ်စေပါသည်။
- ROSE ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက် ဖန်တီးရန် `register_and_mint` ကို တောင်းခံပြီး အရောင်းအ၀ယ်တစ်ခုတွင် Alice သို့ ယူနစ် 250 mintပါ။
- mint အောင်မြင်ကြောင်း အတည်ပြုရန် `client.request(FindAccountAssets)` သို့မဟုတ် `iroha_cli ledger asset list --account <i105-account-id>` မှတဆင့် လက်ကျန်များကို စစ်ဆေးပါ။

## သက်ဆိုင်ရာ SDK လမ်းညွှန်များ

- [Rrust SDK အမြန်စတင်ခြင်း](/sdks/rust)
- [Python SDK အမြန်စတင်ခြင်း](/sdks/python)
- [JavaScript SDK အမြန်စတင်ခြင်း](/sdks/javascript)

[Kotodama အရင်းအမြစ်ကို ဒေါင်းလုဒ်လုပ်ပါ](/norito-snippets/register-and-mint.ko)

```text
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
  #[access(read="*", write="*")]
  kotoage fn register_and_mint() permission(AssetManager) {
    // name, symbol, quantity (precision or supply depending on host), mintable flag
    let name = "rose";
    let symbol = "ROSE";
    let qty = 1000;      // interpretation depends on data model (example only)
    let mintable = 1;    // 1 = mintable, 0 = fixed
    register_asset(name, symbol, qty, mintable);

    // Mint 250 ROSE to Alice
    let to = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```