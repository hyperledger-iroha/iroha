<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 479d552d0f641875518c62059be1084af6ddf99213662a753c73ea57512b8e5f
source_last_modified: "2026-04-02T18:24:28.189405+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/transfer-asset
title: အကောင့်များအကြား ပိုင်ဆိုင်မှုကို လွှဲပြောင်းပါ။
description: SDK အမြန်စတင်မှုများနှင့် လယ်ဂျာလမ်းညွှန်ချက်များကို ထင်ဟပ်စေသည့် ရိုးရှင်းသော ပိုင်ဆိုင်မှုလွှဲပြောင်းခြင်းလုပ်ငန်းအသွားအလာ။
source: examples/transfer/transfer.ko
---

SDK အမြန်စတင်မှုများနှင့် လယ်ဂျာလမ်းညွှန်ချက်များကို ထင်ဟပ်စေသည့် ရိုးရှင်းသော ပိုင်ဆိုင်မှုလွှဲပြောင်းခြင်းလုပ်ငန်းအသွားအလာ။

## လယ်ဂျာရှင်းလင်းချက်

- ပစ်မှတ်ပိုင်ဆိုင်မှုဖြင့် Alice ကို ကြိုတင်ရန်ပုံငွေ (ဥပမာ "စာရင်းသွင်းခြင်းနှင့် mint" အတိုအထွာ သို့မဟုတ် SDK အမြန်စတင်စီးဆင်းမှုများမှတစ်ဆင့်)။
- `AssetTransferRole` ခွင့်ပြုချက်ကို ကျေနပ်စေခြင်းဖြင့် Alice မှ Bob သို့ ယူနစ် 10 လုံးကို ရွှေ့ရန် `do_transfer` ဝင်ခွင့်အမှတ်ကို လုပ်ဆောင်ပါ။
- Query balance (`FindAccountAssets`, `iroha_cli ledger asset list`) သို့မဟုတ် လွှဲပြောင်းမှုရလဒ်ကို စောင့်ကြည့်လေ့လာရန် ပိုက်လိုင်းဖြစ်ရပ်များသို့ စာရင်းသွင်းပါ။

## သက်ဆိုင်ရာ SDK လမ်းညွှန်များ

- [Rrust SDK အမြန်စတင်ခြင်း](/sdks/rust)
- [Python SDK အမြန်စတင်ခြင်း](/sdks/python)
- [JavaScript SDK အမြန်စတင်ခြင်း](/sdks/javascript)

[Kotodama အရင်းအမြစ်ကို ဒေါင်းလုဒ်လုပ်ပါ](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1QG1シタ3vN7ヒzトヘcミLKDCAイ5クエjヤリ2uトユmキユルeJBJW7X2N7"),
      account!("sorauロ1NksツJZミLツスjヨrUphCSホ8Wノスマチモr3ムLセヌヒYqwフノFTMDQE"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```