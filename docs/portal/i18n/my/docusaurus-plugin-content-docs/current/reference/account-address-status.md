---
id: account-address-status
lang: my
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account address compliance
description: Summary of the ADDR-2 fixture workflow and how SDK teams stay in sync.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Canonical ADDR-2 အစုအဝေး (`fixtures/account/address_vectors.json`) ဖမ်းယူသည်
IH58 (ပိုနှစ်သက်သည်)၊ ဖိသိပ်ထားသည် (`sora`၊ ဒုတိယအကောင်းဆုံး၊ တစ်ဝက်/အပြည့် အကျယ်)၊ တံဆိပ်ပေါင်းစုံ၊ နှင့် အနုတ်လက္ခဏာပြကိရိယာများ။
SDK + Torii မျက်နှာပြင်တိုင်းသည် တူညီသော JSON ပေါ်တွင် အားကိုးသဖြင့် မည်သည့်ကုဒ်ဒရိုက်ကိုမဆို သိရှိနိုင်သည်
ထုတ်လုပ်မှုကို မရောက်ခင်မှာ ပျံ့လွင့်နေပါတယ်။ ဤစာမျက်နှာသည် အတွင်းပိုင်းအခြေအနေအကျဉ်းကို ထင်ဟပ်စေသည်။
(`docs/source/account_address_status.md` သည် root repository တွင်) ထို့ကြောင့်ပေါ်တယ်
စာဖတ်သူများသည် mono-repo ကို မတူးဘဲ အလုပ်အသွားအလာကို ကိုးကားနိုင်သည်။

## အစုအဝေးကို ပြန်ထုတ်ပါ သို့မဟုတ် အတည်ပြုပါ။

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

အလံများ

- `--stdout` — ad-hoc စစ်ဆေးခြင်းအတွက် stdout လုပ်ရန် JSON ကို ထုတ်လွှတ်သည်။
- `--out <path>` — မတူညီသောလမ်းကြောင်းတစ်ခုသို့ စာရေးပါ (ဥပမာ၊ diffing သည် locally ပြောင်းလဲသောအခါ)။
- `--verify` — အသစ်ထုတ်လုပ်ထားသော အကြောင်းအရာနှင့် အလုပ်လုပ်သော မိတ္တူကို နှိုင်းယှဉ်ပါ (မရပါ
  `--stdout`) နှင့် ပေါင်းစပ်ထားသည်။

CI အလုပ်အသွားအလာ **Address Vector Drift** သည် `cargo xtask address-vectors --verify` ကို လုပ်ဆောင်သည်
ဘယ်အချိန်မဆို မီးခြစ်၊ မီးစက် သို့မဟုတ် docs သည် သုံးသပ်သူများကို ချက်ချင်းသတိပေးသည်။

## ဘယ်သူက ခံစစ်ကို စားသုံးတာလဲ။

| မျက်နှာပြင် | အတည်ပြုချက် |
|---------|------------|
| သံချေးဒေ - မော်ဒယ် | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (ဆာဗာ) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

ကြိုးဝိုင်းတစ်ခုစီသည် အသွားအပြန် canonical bytes + IH58 + ချုံ့ထားသော (`sora`၊ ဒုတိယအကောင်းဆုံး) ကုဒ်များနှင့်
Norito ပုံစံ အမှားအယွင်းကုဒ်များသည် အနှုတ်လက္ခဏာဆောင်သည့် ကိစ္စများအတွက် တပ်ဆင်မှုနှင့်အတူ လိုင်းတက်နေကြောင်း စစ်ဆေးသည်။

## အလိုအလျောက်စနစ်လိုပါသလား။

ဖြန့်ချိရေးကိရိယာသည် အကူအညီပေးသူနှင့်အတူ script fixture ကိုပြန်လည်စတင်နိုင်သည်။
`scripts/account_fixture_helper.py` သည် ကျမ်းဂန်များကို ရယူခြင်း သို့မဟုတ် အတည်ပြုခြင်း ဖြစ်သည်။
ကော်ပီ/ကူးထည့်ခြင်း အဆင့်များမပါဘဲ အတွဲလိုက်-

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

အကူအညီပေးသူက `--source` အစားထိုးမှုများကို လက်ခံသည် သို့မဟုတ် `IROHA_ACCOUNT_FIXTURE_URL` ကို လက်ခံသည်
ပတ်၀န်းကျင် ပြောင်းလဲနိုင်သော ပြောင်းလဲနိုင်သော SDK CI အလုပ်များသည် ၎င်းတို့၏ နှစ်သက်ရာမှန်ကို ညွှန်ပြနိုင်သည်။
`--metrics-out` ကို ပံ့ပိုးသောအခါ အကူအညီပေးသူက ရေးသည်။
`account_address_fixture_check_status{target=\"…\"}` နှင့်အတူ canonical
SHA-256 digest (`account_address_fixture_remote_info`) ဒါကြောင့် Prometheus textfile
စုဆောင်းသူများနှင့် Grafana ဒက်ရှ်ဘုတ် `account_address_fixture_status` သက်သေပြနိုင်သည်
မျက်နှာပြင်တိုင်းသည် ထပ်တူကျနေပါသည်။ ပစ်မှတ်က `0` ကို သတင်းပို့သည့်အခါတိုင်း သတိပေးချက်။ အဘို့
multi-surface automation သည် wrapper `ci/account_fixture_metrics.sh` ကို အသုံးပြုသည်။
(`--target label=path[::source]` ကို ထပ်ခါတလဲလဲ လက်ခံသည်) ထို့ကြောင့် ခေါ်ဆိုမှုအဖွဲ့များသည် ထုတ်ဝေနိုင်သည်
node-exporter textfile စုဆောင်းသူအတွက် စုစည်းထားသော `.prom` ဖိုင်တစ်ခု။

## အကျဉ်းချုပ် အပြည့်အစုံ လိုအပ်ပါသလား။

အပြည့်အဝ ADDR-2 လိုက်နာမှုအခြေအနေ (ပိုင်ရှင်များ၊ စောင့်ကြည့်ရေးအစီအစဉ်၊ လုပ်ဆောင်ချက်ဖွင့်ထားသည့်အရာများ)
repository တစ်လျှောက် `docs/source/account_address_status.md` တွင်နေထိုင်သည်။
လိပ်စာဖွဲ့စည်းပုံ RFC (`docs/account_structure.md`) နှင့်အတူ။ ဤစာမျက်နှာကို အသုံးပြုပါ။
အမြန်လည်ပတ်မှုသတိပေးချက် အတွင်းကျကျလမ်းညွှန်မှုအတွက် repo docs ကို ဆိုင်းငံ့ထားပါ။