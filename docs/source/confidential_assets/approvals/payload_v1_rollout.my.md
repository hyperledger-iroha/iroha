---
lang: my
direction: ltr
source: docs/source/confidential_assets/approvals/payload_v1_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5fa5e39b0e758b38e27855fcfcae9a6e31817df4fdb9d5394b4b63d2f5164516
source_last_modified: "2026-01-22T14:35:37.742189+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Payload v1 စတင်ဖြန့်ချိခြင်း အတည်ပြုချက် (SDK ကောင်စီ၊ 2026-04-28)။
//!
//! `roadmap.md:M1` လိုအပ်သော SDK ကောင်စီဆုံးဖြတ်ချက်မှတ်စုတိုကို ဖမ်းယူသည်။
//! ကုဒ်ဝှက်ထားသော payload v1 ဖြန့်ချိမှုတွင် စစ်ဆေးမှုမှတ်တမ်း (ပေးပို့နိုင်သော M1.4) ရှိသည်။

# Payload v1 စတင်ရောင်းချခြင်း ဆုံးဖြတ်ချက် (2026-04-28)

- **ဥက္ကဌ-** SDK ကောင်စီခေါင်းဆောင် (M. Takemiya)
- **မဲပေးခြင်းအဖွဲ့ဝင်များ-** Swift Lead၊ CLI Maintainer၊ Confidential Assets TL၊ DevRel WG
- **လေ့လာသူများ-** အစီအစဉ် Mgmt၊ Telemetry Ops

## သွင်းအားစုများကို သုံးသပ်ပြီးပါပြီ။

1. ** Swift bindings & တင်ပြသူများ** — `ShieldRequest`/`UnshieldRequest`၊ async တင်သွင်းသူများနှင့် Tx builder helpers များသည် parity tests များနှင့် docs.【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:389】【IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1006】
2. **CLI ergonomics** — `iroha app zk envelope` ကူညီပေးသူသည် လမ်းပြမြေပုံ ergonomics လိုအပ်ချက်နှင့်အညီ ကုဒ်လုပ်ခြင်း/စစ်ဆေးခြင်း လုပ်ငန်းအသွားအလာများကို ကုဒ်/စစ်ဆေးခြင်း နှင့် ချို့ယွင်းချက်ရှာဖွေခြင်းများကို အကျုံးဝင်သည် ။【crates/iroha_cli/src/zk.rs:1256】
3. ** အဆုံးအဖြတ်ပေးသော ပစ္စည်များ နှင့် တန်းတူညီမျှမှုအစုံများ** — Norito bytes/error မျက်နှာပြင်များကို ထိန်းသိမ်းထားရန် မျှဝေထားသော fixture + Rust/Swift validation ညှိထားသည်။ 【fixtures/confidential/encrypted_payload_v1.json:1】【crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs:1】【IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncrypted”7။

## ဆုံးဖြတ်ချက်

- SDK နှင့် CLI အတွက် ** payload v1 ဖြန့်ချိမှုကို အတည်ပြုပြီး Swift wallets သည် စိတ်ကြိုက်ပိုက်ဆက်ခြင်းမရှိဘဲ လျှို့ဝှက်စာအိတ်များကို အစပြုနိုင်စေသည် ။
**အခြေအနေများ-** 
  - CI ပျံ့လွင့်မှုသတိပေးချက်များ (`scripts/check_norito_bindings_sync.py`) အောက်တွင် တူညီသောပစည်းများကို ထားရှိပါ။
  - `docs/source/confidential_assets.md` (Swift SDK PR မှတစ်ဆင့် အပ်ဒိတ်လုပ်ပြီးသည်) တွင် လုပ်ငန်းလည်ပတ်မှုဆိုင်ရာ ပြခန်းစာအုပ်ကို မှတ်တမ်းတင်ပါ။
  - မည်သည့်ထုတ်လုပ်မှုအလံကိုမျှလှန်ခြင်းမပြုမီ စံချိန်စံညွှန်းသတ်မှတ်ခြင်း + တယ်လီမီတာ အထောက်အထားများ (M2 အောက်တွင် ခြေရာခံသည်)။

## လုပ်ဆောင်ချက်ပစ္စည်းများ

| ပိုင်ရှင် | အမျိုးအမည် | စူးစူး |
|---------|------|-----|
| Swift Lead | GA ရရှိနိုင်မှု + README အတိုအထွာများ | 2026-05-01 |
| CLI Maintainer | `iroha app zk envelope --from-fixture` အကူအညီပေးသူ (ချန်လှပ်ထားနိုင်သည်) | ထည့်ပါ။ Backlog (ပိတ်ဆို့ခြင်းမဟုတ်) |
| DevRel WG | payload v1 ညွှန်ကြားချက်များ | 2026-05-05 |

> **မှတ်ချက်-** ဤမှတ်စုတိုသည် `roadmap.md:2426` တွင် ယာယီ "ဆိုင်းငံ့နေသော ကောင်စီအတည်ပြုချက်" ခေါ်ဆိုမှုအား အစားထိုးပြီး ခြေရာခံသည့်အရာ M1.4 ကို ကျေနပ်စေသည်။ နောက်ဆက်တွဲလုပ်ဆောင်ချက် ပစ္စည်းများ ပိတ်သည့်အခါတိုင်း `status.md` ကို အပ်ဒိတ်လုပ်ပါ။