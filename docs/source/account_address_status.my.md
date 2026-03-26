---
lang: my
direction: ltr
source: docs/source/account_address_status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c2cbb5e965350648a30607bbd0f1588212ee0021b412ec55654993c18cc198e
source_last_modified: "2026-01-28T17:11:30.739162+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## အကောင့်လိပ်စာ လိုက်နာမှုအခြေအနေ (ADDR-2)

အခြေအနေ- 2026-03-30 တွင် လက်ခံခဲ့သည်။  
ပိုင်ရှင်များ- Data Model Team / QA Guild  
လမ်းပြမြေပုံရည်ညွှန်း- ADDR-2 — Dual-Format Compliance Suite

### 1. ခြုံငုံသုံးသပ်ချက်

- Fixture- `fixtures/account/address_vectors.json` (I105 (ဦးစားပေး) + i105 (`sora`၊ ဒုတိယအကောင်းဆုံး) + multisig positive/negative case)။
- နယ်ပယ်- သတ်မှတ်ပြဋ္ဌာန်းထားသော V1 ပေးချေမှုများသည် implicit-default, Local-12, Global registry, နှင့် multisig controllers များကို အမှားအယွင်းအစီအစဥ်အပြည့်အစုံဖြင့် လွှမ်းခြုံထားသည်။
- ဖြန့်ဝေမှု- Rust data-model၊ Torii၊ JS/TS၊ Swift နှင့် Android SDKs တို့တွင် မျှဝေထားသည်။ စားသုံးသူများသွေဖည်ပါက CI ပျက်ကွက်သည်။
- အမှန်တရား၏ရင်းမြစ်- မီးစက်သည် `crates/iroha_data_model/src/account/address/compliance_vectors.rs` တွင်နေထိုင်ပြီး `cargo xtask address-vectors` မှတဆင့် ဖော်ထုတ်ပါသည်။
### 2. ပြန်လည်ထုတ်လုပ်ခြင်းနှင့် အတည်ပြုခြင်း။

```bash
# Write/update the canonical fixture
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Verify the committed fixture matches the generator
cargo xtask address-vectors --verify
```

အလံများ

- `--out <path>` — ad-hoc အစုအဝေးများ ထုတ်လုပ်သောအခါ စိတ်ကြိုက်ရွေးချယ်နိုင်သည် (`fixtures/account/address_vectors.json` သို့ ပုံသေ)။
- `--stdout` — ဒစ်ခ်သို့စာရေးမည့်အစား stdout သို့ JSON ကိုထုတ်လွှတ်ပါ။
- `--verify` — လတ်လတ်ဆတ်ဆတ်ထုတ်လုပ်ထားသော အကြောင်းအရာနှင့် လက်ရှိဖိုင်ကို နှိုင်းယှဉ်ပါ (ပျံ့လွင့်မှုတွင် မြန်ဆန်မှုမရှိပါ၊ `--stdout` နှင့် အသုံးမပြုနိုင်ပါ)။

### 3. Artefact Matrix

| မျက်နှာပြင် | ပြဋ္ဌာန်း | မှတ်စုများ |
|---------|----------------|-------|
| သံချေးဒေ - မော်ဒယ် | `crates/iroha_data_model/tests/account_address_vectors.rs` | JSON ကို ပိုင်းခြားစိတ်ဖြာပြီး ကျမ်းဆိုင်ရာ ပေးဆောင်မှုများကို ပြန်လည်တည်ဆောက်ကာ I105 (ဦးစားပေး)/ ချုံ့ထားသော (`sora`၊ ဒုတိယအကောင်းဆုံး)/ canonical ပြောင်းလဲမှုများ + ဖွဲ့စည်းတည်ဆောက်ပုံ အမှားများကို စစ်ဆေးသည်။ |
| Torii | `crates/iroha_torii/tests/account_address_vectors.rs` | ဆာဗာဘက်ခြမ်းကုဒ်ဒစ်များကို မှန်ကန်ကြောင်းသက်သေပြသောကြောင့် Torii သည် ပုံစံမမှန်သော I105 (ဦးစားပေး)/ချုံ့ထားသော (`sora`၊ ဒုတိယအကောင်းဆုံး) ပေးဆောင်မှုများကို တိကျစွာ ငြင်းဆိုထားသည်။ |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` | Mirrors V1 တန်ဆာပလာများ (I105 ဦးစားပေး/ချုံ့ထားသည် (`sora`) ဒုတိယအကောင်းဆုံး/အပြည့်အဝ) နှင့် အနုတ်ကိစ္စတိုင်းအတွက် Norito ပုံစံ အမှားကုဒ်များကို အတည်ပြုသည်။ |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` | I105 (ဦးစားပေး) လေ့ကျင့်ခန်းများ/ချုံ့ထားသော (`sora`၊ ဒုတိယအကောင်းဆုံး) စကားဝှက်၊ multisig payloads နှင့် Apple ပလပ်ဖောင်းများပေါ်တွင် အမှားအယွင်းများပေါ်နေပါသည်။ |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` | Kotlin/Java ချည်နှောင်မှုများသည် canonical fixture နှင့် လိုက်လျောညီထွေရှိနေစေရန် သေချာစေပါသည်။ |

### 4. Monitoring & Outstanding အလုပ်- အခြေအနေကို အစီရင်ခံခြင်း- ဤစာရွက်စာတမ်းသည် `status.md` နှင့် လမ်းပြမြေပုံမှ ချိတ်ဆက်ထားသောကြောင့် အပတ်စဉ် သုံးသပ်ချက်များသည် ကြံ့ခိုင်မှုအား စစ်ဆေးနိုင်မည်ဖြစ်သည်။
- ဆော့ဖ်ဝဲရေးသားသူပေါ်တယ်အနှစ်ချုပ်- ပြင်ပမျက်နှာစာအကျဉ်းချုပ်အတွက် docs portal (`docs/portal/docs/reference/account-address-status.md`) ရှိ **ကိုးကားချက် → အကောင့်လိပ်စာလိုက်နာမှု** ကို ကြည့်ပါ။
- Prometheus နှင့် ဒက်ရှ်ဘုတ်များ- SDK မိတ္တူကို သင်စစ်ဆေးသည့်အခါတိုင်း၊ `--metrics-out` (နှင့် `--metrics-label`) ဖြင့် အကူအညီပေးသူကို run ခြင်းဖြင့် Prometheus စာသားဖိုင်စုဆောင်းသူသည် I180NI0000 ကို ထည့်သွင်းနိုင်သည်။ Grafana ဒက်ရှ်ဘုတ် **အကောင့်လိပ်စာ တပ်ဆင်မှုအခြေအနေ** (`dashboards/grafana/account_address_fixture_status.json`) သည် မျက်နှာပြင်တစ်ခုစီတွင် ဖြတ်သန်းမှု/ပျက်ကွက်မှု အရေအတွက်များကို ထုတ်ပေးပြီး စာရင်းစစ်ဆိုင်ရာ အထောက်အထားအတွက် Canonical SHA-256 ကို ဖော်ပြသည်။ ပစ်မှတ်က `0` ကို အစီရင်ခံတဲ့အခါ သတိပေးချက်။
- Torii မက်ထရစ်များ- `torii_address_domain_total{endpoint,domain_kind}` သည် ယခုအခါ `torii_address_invalid_total`/`torii_address_local8_total` ကို အောင်မြင်စွာ ခွဲခြမ်းစိတ်ဖြာပြီး အကောင့်တိုင်းအတွက် ပကတိအတိုင်း ထုတ်လွှတ်ပါသည်။ ထုတ်လုပ်မှုတွင် မည်သည့် `domain_kind="local12"` လမ်းကြောင်းကို သတိပေးပြီး ကောင်တာများကို SRE `address_ingest` ဒက်ရှ်ဘုတ်သို့ ပြောင်းထားသောကြောင့် Local-12 အငြိမ်းစားယူဂိတ်တွင် စစ်ဆေးနိုင်သော အထောက်အထားများရှိသည်။
- Fixture helper- `scripts/account_fixture_helper.py` သည် canonical JSON ကို ဒေါင်းလုဒ်လုပ်ခြင်း သို့မဟုတ် အတည်ပြုခြင်း ၊ သို့မှသာ SDK မှ ထွက်ရှိသော အလိုအလျောက်စနစ်သည် Prometheus မက်ထရစ်များကို စိတ်ကြိုက်ရွေးချယ်၍ ကူးယူ/ကူးထည့်ခြင်းမရှိဘဲ အစုအဝေးကို ရယူ/စစ်ဆေးနိုင်ပါသည်။ ဥပမာ-

  ```bash
  # Write the latest fixture to a custom path (defaults to fixtures/account/address_vectors.json)
  python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

  # Fail if an SDK copy drifts from the canonical remote (accepts file:// or HTTPS sources)
  python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

  # Emit Prometheus textfile metrics for dashboards/alerts (writes remote/local digests as labels)
  python3 scripts/account_fixture_helper.py check \\
    --target path/to/sdk/address_vectors.json \\
    --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \\
    --metrics-label android
  ```

  ပစ်မှတ်နှင့် ကိုက်ညီသောအခါတွင် အကူအညီပေးသူက `account_address_fixture_check_status{target="android"} 1` နှင့် SHA-256 အချေအတင်များကို ဖော်ထုတ်ပေးသည့် `account_address_fixture_remote_info` / `account_address_fixture_local_info` တိုင်းထွာများရေးသည်။ ပျောက်ဆုံးနေသောဖိုင်များ `account_address_fixture_local_missing` အစီရင်ခံစာ။
  ပေါင်းစည်းထားသော စာသားဖိုင် (မူလ `artifacts/account_fixture/address_fixture.prom`) ကို ထုတ်လွှတ်ရန် အလိုအလျောက်စနစ်ဖြင့် ထုပ်ပိုးခြင်း- cron/CI မှ `ci/account_fixture_metrics.sh` ကို ခေါ်ပါ။ ထပ်ခါတလဲလဲ `--target label=path` ထည့်သွင်းမှုများကို ကျော်ဖြတ်ပါ (အရင်းအမြစ်ကို အစားထိုးရန်အတွက် ပစ်မှတ်တစ်ခုလျှင် `::https://mirror/...` ကို ထပ်ဖြည့်ပါ) ထို့ကြောင့် Prometheus သည် SDK/CLI မိတ္တူတိုင်းတွင် ပါဝင်သော ဖိုင်တစ်ဖိုင်ကို ခြစ်သည်။ GitHub အလုပ်အသွားအလာ `address-vectors-verify.yml` သည် ဤအကူအညီကို canonical fixture နှင့်ဆန့်ကျင်ပြီး SRE ထည့်သွင်းမှုအတွက် `account-address-fixture-metrics` ကို အပ်လုဒ်လုပ်သည်။