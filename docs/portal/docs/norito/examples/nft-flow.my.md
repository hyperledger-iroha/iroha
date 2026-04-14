<!-- Auto-generated stub for Burmese (my) translation. Replace this content with the full translation. -->

---
lang: my
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09ff44a2df8cbcb9f57017239070a16f5287cbfc59a8289ce54933e84f90a5e8
source_last_modified: "2026-03-26T13:01:47.374572+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/nft-flow
title: Mint၊ လွှဲပြောင်းပြီး NFT ကိုမီးရှို့ပါ။
description: NFT ဘဝစက်ဝန်းတစ်ခုမှ အဆုံးအထိ ဖြတ်သန်းသွားသည်- ပိုင်ရှင်ထံ မြှုပ်နှံခြင်း၊ လွှဲပြောင်းခြင်း၊ မက်တာဒေတာကို တဂ်လုပ်ခြင်း၊ မီးရှို့ခြင်း။
source: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT ဘဝစက်ဝန်းတစ်ခုမှ အဆုံးအထိ ဖြတ်သန်းသွားသည်- ပိုင်ရှင်ထံ မြှုပ်နှံခြင်း၊ လွှဲပြောင်းခြင်း၊ မက်တာဒေတာကို တဂ်လုပ်ခြင်း၊ မီးရှို့ခြင်း။

## လယ်ဂျာရှင်းလင်းချက်

- NFT အဓိပ္ပါယ်ဖွင့်ဆိုချက် (ဥပမာ `n0#wonderland`) သည် အတိုအထွာတွင်အသုံးပြုထားသော ပိုင်ရှင်/လက်ခံသူအကောင့်များ (Alice အတွက် `<i105-account-id>`၊ `<i105-account-id>`) နှင့်အတူ ရှိနေကြောင်း သေချာပါစေ။
- NFT ကို mint ရန် `nft_issue_and_transfer` ကို ခေါ်၍ Alice မှ Bob သို့ လွှဲပြောင်းပြီး ထုတ်ပေးမှုကို ဖော်ပြသည့် မက်တာဒေတာအလံကို ပူးတွဲပါ။
- လွှဲပြောင်းမှုကိုစစ်ဆေးရန် `iroha_cli ledger nft list --account <id>` သို့မဟုတ် ညီမျှသော SDK ဖြင့် NFT လယ်ဂျာအခြေအနေအား စစ်ဆေးပါ၊ ထို့နောက် မီးလောင်မှုညွှန်ကြားချက်ကို လုပ်ဆောင်သည်နှင့် ပိုင်ဆိုင်မှုကို ဖယ်ရှားကြောင်း အတည်ပြုပါ။

## သက်ဆိုင်ရာ SDK လမ်းညွှန်များ

- [Rrust SDK အမြန်စတင်ခြင်း](/sdks/rust)
- [Python SDK အမြန်စတင်ခြင်း](/sdks/python)
- [JavaScript SDK အမြန်စတင်ခြင်း](/sdks/javascript)

[Kotodama အရင်းအမြစ်ကို ဒေါင်းလုဒ်လုပ်ပါ](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  #[access(read="*", write="*")]
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```