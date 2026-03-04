---
lang: my
direction: ltr
source: docs/genesis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9c2eab4379aa346ab7d111e1c51c0230238f260647187f1a33c1819640b9bf2c
source_last_modified: "2026-01-28T14:25:37.056140+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ကမ္ဘာဦးဖွဲ့စည်းမှု

`genesis.json` ဖိုင်တစ်ခုသည် Iroha ကွန်ရက်စတင်ချိန်တွင် လုပ်ဆောင်သည့် ပထမဆုံး ငွေပေးငွေယူများကို သတ်မှတ်သည်။ ဖိုင်သည် ဤအကွက်များပါရှိသော JSON အရာဝတ္ထုဖြစ်သည်-

- `chain` - ထူးခြားသော ကွင်းဆက်အမှတ်အသား။
- `executor` (ချန်လှပ်ထားနိုင်သည်) - executor bytecode (`.to`) သို့ လမ်းကြောင်း။ ပစ္စုပ္ပန်၊
  ဥပါဒ်တွင် ပထမဆုံး အရောင်းအ၀ယ်အဖြစ် အဆင့်မြှင့်တင်ရန် ညွှန်ကြားချက်တစ်ခု ပါဝင်သည်။ ချန်လှပ်ထားရင်၊
  အဆင့်မြှင့်ခြင်း မလုပ်ဆောင်ဘဲ တပ်ဆင်ထားသော executor ကို အသုံးပြုထားသည်။
- `ivm_dir` - IVM bytecode libraries များပါရှိသော လမ်းညွှန်။ ချန်လှပ်ထားလျှင် ပုံသေသည် `"."` ဖြစ်သည်။
- `consensus_mode` – မန်နီးဖက်စ်တွင် ကြော်ငြာထားသော အများဆန္ဒမုဒ်။ လိုအပ်သည်; အများသူငှာ Sora Nexus ဒေတာအာကာသအတွက် `"Npos"` သို့မဟုတ် အခြားသော Iroha3 ဒေတာနေရာများအတွက် `"Permissioned"`/`"Npos"` ကို အသုံးပြုပါ။ Iroha2 သည် `"Permissioned"` သို့ ပုံသေဖြစ်သည်။
- `transactions` - စဉ်ဆက်မပြတ်လုပ်ဆောင်ခဲ့သော ဥပါဒ်အရောင်းအဝယ်များစာရင်း။ ထည့်သွင်းမှုတိုင်းတွင်-
  - `parameters` - ကနဦးကွန်ရက် ကန့်သတ်ချက်များ။
  - `instructions` – ဖွဲ့စည်းထားသော Norito ညွှန်ကြားချက်များ (ဥပမာ၊ `{ "Register": { "Domain": { "id": "wonderland" }}}`)။ Raw byte array များကို လက်မခံပါ၊ `SetParameter` ညွှန်ကြားချက်များကို ဤနေရာတွင် ပယ်ချပါသည်—`parameters` ပိတ်ဆို့ခြင်းမှတစ်ဆင့် မျိုးစေ့ပါရာမီတာများကို ညွှန်ကြားချက်များကို ပုံမှန်ဖြစ်အောင်/လက်မှတ်ထိုးခွင့်ပြုပါ။
  - `ivm_triggers` - IVM bytecode executable များဖြင့် အစပျိုးသည်။
  - `topology` - ကနဦး မျိုးတူစုတူ ပေါ်လစီ။ ထည့်သွင်းမှုတစ်ခုစီသည် peer id နှင့် PoP ကို ​​အတူတကွထားရှိသည်- `{ "peer": "<public_key>", "pop_hex": "<hex>" }`။ `pop_hex` ကို ရေးဖွဲ့နေစဉ် ချန်လှပ်ထားနိုင်သော်လည်း လက်မှတ်မထိုးမီတွင် ရှိနေရပါမည်။
- `crypto` – `iroha_config.crypto` (`default_hash`, `allowed_signing`, `allowed_curve_ids`, `sm2_distid_default`,018NI00000062X,018NI000000620X၊ `allowed_curve_ids` မှန်များသည် `crypto.curves.allowed_curve_ids` ဖြစ်သောကြောင့် မန်နီးဖက်စ်များသည် မည်သည့် controller ကွေးကောက်ခြင်းကို လက်ခံသည်ဖြစ်စေ အစုအဝေးကို ကြော်ငြာနိုင်သည်။ Tooling သည် SM ပေါင်းစပ်မှုများကို တွန်းအားပေးသည်- `sm2` ၏ စာရင်းတွင် hash ကို `sm3-256` သို့ ပြောင်းရမည်ဖြစ်ပြီး `sm` အင်္ဂါရပ်မပါဘဲ စုစည်းထားသော တည်ဆောက်မှုများသည် `sm2` တွင် လုံးဝငြင်းဆိုထားသည်။ Normalization သည် `crypto_manifest_meta` စိတ်ကြိုက်ဘောင်တစ်ခုကို လက်မှတ်ရေးထိုးထားသော ဥပါဒ်ထဲသို့ ထိုးသွင်းသည်။ ထိုးသွင်းထားသော payload သည် ကြော်ငြာထားသောလျှပ်တစ်ပြက်ရိုက်ချက်အား သဘောမတူပါက node များစတင်ရန်ငြင်းဆိုသည်။

ဥပမာ (`kagami genesis generate default --consensus-mode npos` အထွက်၊ ညွှန်ကြားချက်များကို ဖြတ်တောက်ထားသည်-

```json
{
  "chain": "00000000-0000-0000-0000-000000000000",
  "ivm_dir": "defaults",
  "transactions": [
    {
      "parameters": { "sumeragi": { "block_time_ms": 2000 } },
      "instructions": [
        { "Register": { "Domain": { "id": "wonderland" } } }
      ],
      "ivm_triggers": [],
      "topology": [
        {
          "peer": "ed25519:...",
          "pop_hex": "ab12cd..."
        }
      ]
    }
  ],
  "consensus_mode": "Npos",
  "crypto": {
    "default_hash": "blake2b-256",
    "allowed_signing": ["ed25519", "secp256k1"],
    "allowed_curve_ids": [1],
    "sm2_distid_default": "1234567812345678",
    "sm_openssl_preview": false
  }
}
```

### SM2/SM3 အတွက် `crypto` ဘလောက်ကို ကြည့်ပါ။

အဆင့်တစ်ဆင့်တွင် သော့ချက်စာရင်းနှင့် အဆင်သင့်ထည့်သွင်းနိုင်သော ဖွဲ့စည်းမှုပုံစံအတိုအထွာကို ထုတ်လုပ်ရန် xtask အကူအညီပေးသူကို အသုံးပြုပါ-

```bash
cargo xtask sm-operator-snippet \
  --distid CN12345678901234 \
  --json-out sm2-key.json \
  --snippet-out client-sm2.toml
```

ယခု `client-sm2.toml` ပါရှိသည်-

```toml
# Account key material
public_key = "sm2:8626530010..."
private_key = "A333F581EC034C1689B750A827E150240565B483DEB28294DDB2089AD925A569"
# public_key_pem = """\
-----BEGIN PUBLIC KEY-----
...
-----END PUBLIC KEY-----
"""
# private_key_pem = """\
-----BEGIN PRIVATE KEY-----
...
-----END PRIVATE KEY-----
"""

[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "secp256k1", "sm2"]  # remove "sm2" to stay in verify-only mode
allowed_curve_ids = [1]               # add new curve ids (e.g., 15 for SM2) when controllers are allowed
sm2_distid_default = "CN12345678901234"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```

`public_key`/`private_key` တန်ဖိုးများကို အကောင့်/ကလိုင်းယင့် ဖွဲ့စည်းမှုပုံစံသို့ ကူးယူပြီး `crypto` ၏ `crypto` ဘလောက်ကို အပ်ဒိတ်လုပ်ကာ ၎င်းသည် အတိုအထွာနှင့် ကိုက်ညီသောကြောင့် ၎င်းသည် အတိုအထွာနှင့် ကိုက်ညီသည် (ဥပမာ၊ Kagami တွင် ထည့်ရန် I18NI00000000 ဟု သတ်မှတ်သည် `"sm2"` မှ `allowed_signing` နှင့် ညာဘက် `allowed_curve_ids` ပါ၀င်သည်)။ Kagami သည် hash/curve settings နှင့် signing list ကွဲလွဲနေသည့် သရုပ်ကို ငြင်းဆိုပါမည်။

> ** အကြံပြုချက်-** အထွက်ကို စစ်ဆေးလိုသောအခါတွင် `--snippet-out -` ဖြင့် stdout လုပ်ရန် snippet ကို တိုက်ရိုက်ကြည့်ရှုပါ။ stdout တွင် သော့စာရင်းကို ထုတ်လွှတ်ရန် `--json-out -` ကို အသုံးပြုပါ။

အောက်ခြေအဆင့် CLI ညွှန်ကြားချက်များကို ကိုယ်တိုင်မောင်းနှင်လိုပါက၊ ညီမျှသောစီးဆင်းမှုသည်-

```bash
# 1. Produce deterministic key material (writes JSON to disk)
cargo run -p iroha_cli --features sm -- \
  crypto sm2 keygen \
  --distid CN12345678901234 \
  --output sm2-key.json

# 2. Re-hydrate the snippet that can be pasted into client/config files
cargo run -p iroha_cli --features sm -- \
  crypto sm2 export \
  --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
  --distid CN12345678901234 \
  --snippet-output client-sm2.toml \
  --emit-json --quiet
```

> ** အကြံပြုချက်-** `jq` ကို manual copy/paste အဆင့်ကို သိမ်းဆည်းရန် အထက်တွင် အသုံးပြုထားသည်။ မရရှိနိုင်ပါက `sm2-key.json` ကိုဖွင့်ပါ၊ `private_key_hex` အကွက်ကို ကူးယူပြီး ၎င်းကို `crypto sm2 export` သို့ တိုက်ရိုက်ပေးပို့ပါ။

> **ရွှေ့ပြောင်းခြင်းလမ်းညွှန်-** ရှိပြီးသားကွန်ရက်တစ်ခုကို SM2/SM3/SM4 သို့ ပြောင်းလဲသည့်အခါ၊ လိုက်နာပါ။
> [`docs/source/crypto/sm_config_migration.md`](source/crypto/sm_config_migration.md)
> အလွှာလိုက် `iroha_config` ကို အစားထိုးခြင်း၊ ပြန်လည်ဆန်းသစ်ခြင်းနှင့် ပြန်လှည့်ခြင်းအတွက်
> စီစဉ်ခြင်း။

## ထုတ်လုပ်ပြီး အတည်ပြုပါ။

1. နမူနာပုံစံတစ်ခု ဖန်တီးပါ-
   ```bash
   cargo run -p iroha_kagami -- genesis generate \
     [--executor <path/to/executor.to>] \
     --consensus-mode npos \
     --ivm-dir <ivm/dir> \
     --genesis-public-key <PUBLIC_KEY> > genesis.json
   ```
`--consensus-mode` သည် `parameters` ဘလောက်ထဲသို့ မျိုးစေ့များ Kagami ၏ သဘောတူညီမှုဘောင်များကို ထိန်းချုပ်သည်။ အများသူငှာ Sora Nexus ဒေတာအာကာသသည် `npos` လိုအပ်ပြီး အဆင့်လိုက်ဖြတ်တောက်မှုများကို မပံ့ပိုးပါ။ အခြား Iroha3 ဒေတာနေရာလွတ်များသည် ခွင့်ပြုချက် သို့မဟုတ် NPoS ကို အသုံးပြုနိုင်သည်။ Iroha2 သည် `permissioned` သို့ ပုံသေသတ်မှတ်ထားပြီး `npos` ကို `--next-consensus-mode`/`--mode-activation-height` မှတစ်ဆင့် `--mode-activation-height` သို့ အဆင့်သတ်မှတ်နိုင်သည်။ `npos` ကို ရွေးချယ်သောအခါ၊ Kagami သည် NPoS စုဆောင်းသူအား အမာခံထွက်ခြင်း၊ ရွေးကောက်ပွဲ မူဝါဒနှင့် ပြန်လည်ပြင်ဆင်ခြင်း windows များကို မောင်းနှင်ပေးသော `sumeragi_npos_parameters` ပါဝိုက်ကို စေ့စပ်ထားသည်။ ပုံမှန်ပြုလုပ်ခြင်း/လက်မှတ်ထိုးခြင်းမှ ၎င်းတို့ကို `SetParameter` ညွှန်ကြားချက်များအဖြစ်သို့ ပြောင်းလဲပေးပါသည်။
2. ရွေးချယ်နိုင်သော `genesis.json` ကို တည်းဖြတ်ပါ၊ ထို့နောက် အတည်ပြုပြီး လက်မှတ်ထိုးပါ-
   ```bash
   cargo run -p iroha_kagami -- genesis sign genesis.json \
     --public-key <PUBLIC_KEY> \
     --private-key <PRIVATE_KEY> \
     --out-file genesis.signed.nrt
   ```

   SM2/SM3/SM4 အဆင်သင့်ရှိသော မန်နီးဖက်စ်များကို ထုတ်လွှတ်ရန်၊ `--default-hash sm3-256` ကို ဖြတ်ပြီး `--allowed-signing sm2` ကို ထည့်သွင်းပါ (နောက်ထပ် algorithms အတွက် `--allowed-signing` ကို ထပ်လုပ်ပါ)။ ပုံသေခွဲခြားသတ်မှတ်မှုအား အစားထိုးရန် လိုအပ်ပါက `--sm2-distid-default <ID>` ကို အသုံးပြုပါ။

   `irohad` ကို `--genesis-manifest-json` (လက်မှတ်မထိုးထားသော genesis block) ဖြင့် စတင်သောအခါ node သည် ၎င်း၏ runtime crypto configuration ကို manifest မှ အလိုအလျောက် ထုတ်ပေးပါသည်။ အကယ်၍ သင်သည် genesis block တစ်ခုကို ထောက်ပံ့ပေးပါက၊ manifest နှင့် config သည် အတိအကျတူညီနေရပါမည်။

- အတည်ပြုမှတ်စုများ
  - Kagami သည် `consensus_handshake_meta`၊ `confidential_registry_root`၊ နှင့် `crypto_manifest_meta` ကို `SetParameter` ညွှန်ကြားချက်များအဖြစ် ပုံမှန်/လက်မှတ်ရေးထိုးထားသော ပိတ်ဆို့ခြင်း။ `irohad` သည် အဆိုပါ payload များမှ အများဆန္ဒလက်ဗွေကို ပြန်လည်တွက်ချက်ပြီး လက်ဆွဲမက်တာဒေတာ သို့မဟုတ် crypto လျှပ်တစ်ပြက်ရိုက်ချက်သည် ကုဒ်လုပ်ထားသော ကန့်သတ်ဘောင်များကို သဘောမတူပါက စတင်လုပ်ဆောင်မှု မအောင်မြင်ပါ။ ဒါတွေကို မန်နီးဖက်စ်မှာ `instructions` ထဲက သိမ်းထားပါ။ ၎င်းတို့ကို အလိုအလျောက် ထုတ်ပေးပါသည်။
- ပုံမှန်လုပ်ကွက်ကို စစ်ဆေးပါ။
  - သော့ချိတ်ကို မပံ့ပိုးဘဲ နောက်ဆုံးမှာယူထားသော ငွေပေးငွေယူများ (ထိုးသွင်းထားသော မက်တာဒေတာ အပါအဝင်) `kagami genesis normalize genesis.json --format text` ကို run ပါ။
  - ကွဲပြားခြင်း သို့မဟုတ် သုံးသပ်ချက်များအတွက် သင့်လျော်သော ဖွဲ့စည်းတည်ဆောက်ပုံမြင်ကွင်းကို စွန့်ပစ်ရန် `--format json` ကို အသုံးပြုပါ။

`kagami genesis sign` သည် JSON မှန်ကန်ကြောင်း စစ်ဆေးပြီး I18NI000000117X မှတစ်ဆင့် အသုံးပြုရန် အဆင်သင့်ဖြစ်နေသော Norito-encoded ဘလောက်တစ်ခုကို ထုတ်လုပ်သည်။ ရလာဒ် `genesis.signed.nrt` သည် canonical ဝါယာကြိုးပုံစံတွင်ရှိပြီးဖြစ်သည်- ဗားရှင်း byte ၏နောက်တွင် payload layout ကိုဖော်ပြသည့် Norito ခေါင်းစီးဖြင့်။ ဤဘောင်ခတ်ထားသော အထွက်ကို အမြဲဖြန့်ဝေပါ။ လက်မှတ်ထိုးထားသော payloads အတွက် `.nrt` နောက်ဆက်တွဲကို ဦးစားပေးပါ။ အကယ်၍ သင်သည် genesis တွင် executor ကို အဆင့်မြှင့်ရန် မလိုအပ်ပါက၊ သင်သည် `executor` အကွက်ကို ချန်လှပ်ပြီး `.to` ဖိုင်ကို ပံ့ပိုးပေးမှုကို ကျော်သွားနိုင်ပါသည်။

NPoS ကို လက်မှတ်ထိုးသည့်အခါ (`--consensus-mode npos` သို့မဟုတ် Iroha2 သီးသန့် ဖြတ်တောက်ထားသော ဖြတ်တောက်မှုများ)၊ `kagami genesis sign` သည် `sumeragi_npos_parameters` payload လိုအပ်ပါသည်။ ၎င်းကို `kagami genesis generate --consensus-mode npos` ဖြင့် ထုတ်လုပ်ပါ သို့မဟုတ် ဘောင်ကို ကိုယ်တိုင်ထည့်ပါ။
မူရင်းအားဖြင့် `kagami genesis sign` သည် မန်နီးဖက်စ်၏ `consensus_mode` ကို အသုံးပြုသည်။ ၎င်းကို အစားထိုးရန် `--consensus-mode` ကို ကျော်ပါ။

## ကမ္ဘာဦးဘာလုပ်နိုင်သလဲ။

ကမ္ဘာဦးကျမ်းသည် အောက်ပါလုပ်ငန်းများကို ပံ့ပိုးပေးသည်။ Kagami သည် ၎င်းတို့အား ကောင်းစွာသတ်မှတ်ထားသော အစီအစဥ်တစ်ခုအဖြစ် စုစည်းထားသောကြောင့် ရွယ်တူများသည် တူညီသော sequence ကို အဆုံးအဖြတ်ပေးသည်။

- ကန့်သတ်ချက်များ- Sumeragi အတွက် ကနဦးတန်ဖိုးများ (ပိတ်ဆို့ခြင်း/လုပ်ဆောင်ရမည့်အချိန်များ၊ ပျံ့လွင့်မှု)၊ ပိတ်ဆို့ခြင်း (max txs)၊ ငွေပေးငွေယူ (အမြင့်ဆုံး ညွှန်ကြားချက်များ၊ bytecode အရွယ်အစား)၊ အမှုဆောင်အရာရှိနှင့် စမတ်စာချုပ် ကန့်သတ်ချက်များ (လောင်စာဆီ၊ မှတ်ဉာဏ်၊ အတိမ်အနက်) နှင့် စိတ်ကြိုက်ကန့်သတ်ချက်များ။ Kagami အစေ့များ `Sumeragi::NextMode` နှင့် `sumeragi_npos_parameters` payload (NPoS ရွေးကောက်ပွဲ၊ ပြန်လည်ပြင်ဆင်မှု) ကို `parameters` ပိတ်ဆို့ခြင်းမှတစ်ဆင့် စတင်လုပ်ဆောင်နိုင်သောကြောင့် စတင်ခြင်းလုပ်ငန်းသည် ကွင်းဆက်အခြေအနေမှ သဘောတူညီမှုခလုတ်များကို အသုံးပြုနိုင်သည်။ လက်မှတ်ရေးထိုးထားသော ဘလောက်သည် ထုတ်လုပ်ထားသော `SetParameter` ညွှန်ကြားချက်များကို သယ်ဆောင်သည်။
- မူရင်းညွှန်ကြားချက်များ- မှတ်ပုံတင်/မှတ်ပုံတင်ခြင်း ဒိုမိန်း၊ အကောင့်၊ ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက်။ Mint / Burn / Transfer ပိုင်ဆိုင်မှု; ဒိုမိန်းလွှဲပြောင်းခြင်းနှင့် ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက် ပိုင်ဆိုင်မှု၊ မက်တာဒေတာကို မွမ်းမံပါ။ ခွင့်ပြုချက်များနှင့် အခန်းကဏ္ဍများကို ပေးသည်။
- IVM အစပျိုးများ- IVM bytecode ကိုလုပ်ဆောင်သည့် အစပျိုးများ မှတ်ပုံတင်ပါ (`ivm_triggers` ကိုကြည့်ပါ)။ Triggers ၏ executable များသည် `ivm_dir` နှင့် ဆက်စပ်မှုကို ဖြေရှင်းပါသည်။
- Topology- မည်သည့်ငွေပေးငွေယူအတွင်းတွင်မဆို `topology` array မှတစ်ဆင့် ကနဦးလုပ်ဖော်ကိုင်ဖက်အစုအဝေးကို ပေးဆောင်ပါ (အများအားဖြင့် ပထမ သို့မဟုတ် နောက်ဆုံးတစ်ခု)။ တစ်ခုစီသည် `{ "peer": "<public_key>", "pop_hex": "<hex>" }` ဖြစ်သည်။ `pop_hex` ကို ရေးဖွဲ့နေစဉ် ချန်လှပ်ထားနိုင်သော်လည်း လက်မှတ်မထိုးမီတွင် ရှိနေရပါမည်။
- Executor Upgrade (ချန်လှပ်ထားနိုင်သည်) - အကယ်၍ `executor` ရှိနေပါက၊ ဥပါဒ်က ပထမဆုံး ငွေပေးငွေယူအဖြစ် Upgrade ညွှန်ကြားချက်တစ်ခုကို ထည့်သွင်းပါ။ သို့မဟုတ်ပါက ဥပါဒ်သည် ကန့်သတ်ချက်များ/ညွှန်ကြားချက်များဖြင့် တိုက်ရိုက်စတင်သည်။

### ငွေလွှဲအော်ဒါမှာယူခြင်း။

သဘောတရားအရ၊ ဥပါဒ် အရောင်းအဝယ်များကို ဤအစီအစဥ်အတိုင်း လုပ်ဆောင်သည်-

1) (ရွေးချယ်နိုင်သည်) Executor အဆင့်မြှင့်တင်ခြင်း။
2) `transactions` ရှိ ငွေပေးငွေယူတစ်ခုစီအတွက်၊
   - ကန့်သတ်ချက်မွမ်းမံမှုများ
   - ဇာတိညွှန်ကြားချက်
   - IVM အစပျိုး မှတ်ပုံတင်မှုများ
   - Topology ထည့်သွင်းမှုများ

Kagami နှင့် node ကုဒ်သည် ဤအော်ဒါမှာခြင်းကို သေချာစေရန်၊ ဥပမာအားဖြင့်၊ တူညီသောငွေပေးငွေယူတွင် နောက်ဆက်တွဲညွှန်ကြားချက်များရှေ့တွင် ကန့်သတ်ချက်များသက်ရောက်စေရန်။

## အလုပ်အသွားအလာကို အကြံပြုထားသည်။

- Kagami ဖြင့် နမူနာပုံစံမှ စတင်ပါ။
  - Built-in ISI သီးသန့်- `kagami genesis generate --ivm-dir <dir> --genesis-public-key <PK> --consensus-mode npos > genesis.json` (Sora Nexus အများသူငှာဒေတာနေရာ၊ `--consensus-mode permissioned` ကို Iroha2 သို့မဟုတ် သီးသန့် Iroha3 အတွက် `--consensus-mode permissioned` ကိုသုံးပါ။
  - စိတ်ကြိုက် executor အဆင့်မြှင့်ခြင်းဖြင့် (ချန်လှပ်ထားနိုင်သည်): `--executor <path/to/executor.to>` ကို ထည့်ပါ။
  - Iroha2-only- NPoS သို့ အနာဂတ်ဖြတ်တောက်မှုကို လုပ်ဆောင်ရန်၊ `--next-consensus-mode npos --mode-activation-height <HEIGHT>` ကို ကျော်ဖြတ်ပါ (လက်ရှိမုဒ်အတွက် `--consensus-mode permissioned` ကို သိမ်းထားပါ)။
- `<PK>` သည် `--features gost` ဖြင့်တည်ဆောက်သောအခါ TC26 GOST မျိုးကွဲများအပါအဝင် `iroha_crypto::Algorithm` မှအသိအမှတ်ပြုထားသော မည်သည့် multihash မဆိုဖြစ်သည် (ဥပမာ `gost3410-2012-256-paramset-a:...`)။
- တည်းဖြတ်နေစဉ်အတွင်း အတည်ပြုပါ- `kagami genesis validate genesis.json`
- ဖြန့်ကျက်ရန် ဆိုင်းဘုတ်- `kagami genesis sign genesis.json --public-key <PK> --private-key <SK> --out-file genesis.signed.nrt`
- ရွယ်တူများကို စီစဉ်သတ်မှတ်ပါ- `genesis.file` ကို လက်မှတ်ထိုးထားသော Norito ဖိုင် (ဥပမာ၊ `genesis.signed.nrt`) နှင့် `genesis.public_key` ကို လက်မှတ်ထိုးရာတွင် အသုံးပြုသည့် တူညီသော `<PK>` သို့ သတ်မှတ်ပါ။မှတ်စုများ-
- Kagami ၏ "မူလ" ပုံစံသည် နမူနာဒိုမိန်းနှင့် အကောင့်များကို မှတ်ပုံတင်ပြီး ပိုင်ဆိုင်မှုအနည်းငယ်ကို ထုတ်ယူကာ တပ်ဆင်ပါရှိ ISI များသာ အသုံးပြု၍ အနည်းဆုံးခွင့်ပြုချက်ပေးသည် - `.to` မလိုအပ်ပါ။
- အကယ်၍ သင်သည် executor အဆင့်မြှင့်တင်မှုတစ်ခုပါဝင်ပါက၊ ၎င်းသည် ပထမဆုံးငွေပေးငွေယူဖြစ်ရပါမည်။ Kagami သည် ထုတ်လုပ်/လက်မှတ်ထိုးသည့်အခါ ၎င်းကို ပြဋ္ဌာန်းသည်။
- လက်မှတ်မထိုးမီ မမှန်ကန်သော `Name` တန်ဖိုးများ (ဥပမာ၊ နေရာလွတ်) နှင့် ပုံစံမမှန်သော ညွှန်ကြားချက်များကို ရယူရန် `kagami genesis validate` ကို သုံးပါ။

## Docker/Swarm ဖြင့် လုပ်ဆောင်နေသည်။

ပံ့ပိုးပေးထားသည့် Docker Compose နှင့် Swarm tooling သည် ကိစ္စနှစ်ခုလုံးကို ကိုင်တွယ်သည်-

- executor မပါဘဲ- ရေးဖွဲ့ခြင်းအမိန့်သည် ပျောက်ဆုံးနေသော/ဗလာ `executor` အကွက်ကို ဖယ်ထုတ်ပြီး ဖိုင်ကို လက်မှတ်ထိုးပါ။
- executor ဖြင့်- ၎င်းသည် ကွန်တိန်နာအတွင်းရှိ absolute path သို့ ဆွေမျိုး executor လမ်းကြောင်းကို ဖြေရှင်းပေးပြီး ဖိုင်ကို အမှတ်အသားပြုသည်။

၎င်းသည် ကြိုတင်တည်ဆောက်ထားသော IVM နမူနာများမပါဘဲ စက်များတွင် ဖွံ့ဖြိုးတိုးတက်မှုကို ရိုးရှင်းစေပြီး လိုအပ်သည့်အခါတွင် executor အဆင့်မြှင့်တင်မှုများကို ခွင့်ပြုဆဲဖြစ်သည်။