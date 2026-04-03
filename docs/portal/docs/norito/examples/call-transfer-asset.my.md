<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dcd8de175a7c5172158a03e1a25b254c90a11e62c173f95b8d9e4a387df6ba09
source_last_modified: "2026-03-26T13:01:47.372931+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/call-transfer-asset
title: Kotodama မှ လက်ခံသူလွှဲပြောင်းမှုကို တောင်းဆိုပါ။
description: Kotodama entrypoint သည် host `transfer_asset` instruction ကို inline metadata validation ဖြင့် မည်သို့ခေါ်ဆိုနိုင်ကြောင်း သရုပ်ပြသည်။
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Kotodama entrypoint သည် host `transfer_asset` instruction ကို inline metadata validation ဖြင့် မည်သို့ခေါ်ဆိုနိုင်ကြောင်း သရုပ်ပြသည်။

## လယ်ဂျာရှင်းလင်းချက်

- စာချုပ်ဆိုင်ရာအခွင့်အာဏာ (ဥပမာ `<i105-account-id>`) ကို ပိုင်ဆိုင်မှုလွှဲပြောင်းပေးပြီး အခွင့်အာဏာကို `CanTransfer` အခန်းကဏ္ဍ သို့မဟုတ် ညီမျှသောခွင့်ပြုချက်ဖြင့် ရန်ပုံငွေထည့်ပါ။
- စာချုပ်အကောင့်မှ Bob (`<i105-account-id>`) သို့ 5 ယူနစ်များ လွှဲပြောင်းရန် `call_transfer_asset` ကို ခေါ်ဆိုပါ၊ ကွင်းဆက်အလိုအလျောက်စနစ်ဖြင့် လက်ခံဆောင်ရွက်ပေးသောခေါ်ဆိုမှုများကို ခြုံငုံနိုင်စေမည့် နည်းလမ်းကို ထင်ဟပ်စေသည်။
- `FindAccountAssets` သို့မဟုတ် `iroha_cli ledger asset list --account <i105-account-id>` မှတစ်ဆင့် လက်ကျန်ငွေများကို စစ်ဆေးပြီး မက်တာဒေတာစောင့်သည် လွှဲပြောင်းသည့်အကြောင်းအရာကို မှတ်တမ်းတင်ထားကြောင်း အတည်ပြုရန် ဖြစ်ရပ်များကို စစ်ဆေးပါ။

## သက်ဆိုင်ရာ SDK လမ်းညွှန်များ

- [Rrust SDK အမြန်စတင်ခြင်း](/sdks/rust)
- [Python SDK အမြန်စတင်ခြင်း](/sdks/python)
- [JavaScript SDK အမြန်စတင်ခြင်း](/sdks/javascript)

[Kotodama အရင်းအမြစ်ကို ဒေါင်းလုဒ်လုပ်ပါ](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
      account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```