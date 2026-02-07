---
lang: my
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Client API Configuration Reference

ဤစာတမ်းသည် Torii ဖောက်သည်-မျက်နှာပေး ဖွဲ့စည်းမှုပုံစံခလုတ်များကို ခြေရာခံသည်
`iroha_config::parameters::user::Torii` မှတဆင့် မျက်နှာပြင်များ။ အောက်ပါအပိုင်း
NRPC-1 အတွက် မိတ်ဆက်ထားသော Norito-RPC သယ်ယူပို့ဆောင်ရေး ထိန်းချုပ်မှုများကို အာရုံစိုက်ပါ။ အနာဂတ်
client API ဆက်တင်များသည် ဤဖိုင်ကို တိုးချဲ့သင့်သည်။

### `torii.transport.norito_rpc`

| သော့ | ရိုက် | ပုံသေ | ဖော်ပြချက် |
|--|------|---------|-------------|
| `enabled` | `bool` | `true` | binary Norito စကားဝှက်ကို ဖွင့်ပေးသည့် မာစတာခလုတ်။ `false`၊ Torii သည် Norito-RPC တောင်းဆိုမှုတိုင်းကို `403 norito_rpc_disabled` ဖြင့် ငြင်းပယ်သည်။ |
| `stage` | `string` | `"disabled"` | ထုတ်လွှတ်မှုအဆင့်- `disabled`၊ `canary` သို့မဟုတ် `ga`။ ဝင်ခွင့်ဆိုင်ရာ ဆုံးဖြတ်ချက်များနှင့် `/rpc/capabilities` ရလဒ်များကို အဆင့်ဆင့်မောင်းနှင်သည်။ |
| `require_mtls` | `bool` | `false` | Norito-RPC သယ်ယူပို့ဆောင်ရေးအတွက် mTLS ကို တွန်းအားပေးသည်- `true`၊ Torii သည် mTLS အမှတ်အသား ခေါင်းစီး (ဥပမာ) I18000000 မပါသော Norito-RPC တောင်းဆိုချက်များကို ငြင်းပယ်သည့်အခါ အလံသည် `/rpc/capabilities` မှတစ်ဆင့် ပေါ်နေသောကြောင့် SDK များသည် ပုံသေသတ်မှတ်မှု မှားယွင်းသော ပတ်ဝန်းကျင်များတွင် သတိပေးနိုင်သည်။ |
| `allowed_clients` | `array<string>` | `[]` | Canary ခွင့်ပြုစာရင်း။ `stage = "canary"` တွင် ဤစာရင်းတွင်ပါရှိသော `X-API-Token` ခေါင်းစီးပါသော တောင်းဆိုမှုများကိုသာ လက်ခံပါသည်။ |

နမူနာဖွဲ့စည်းပုံ-

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

ဇာတ်ခုံ ဝေါဟာရ-

- **disabled** — Norito-RPC `enabled = true` ဆိုလျှင်တောင် မရနိုင်ပါ။ ဟိုဟာ
  `403 norito_rpc_disabled` ကို လက်ခံရယူပါ။
- **canary** — တောင်းဆိုချက်များတွင် တစ်ခုနှင့် ကိုက်ညီသော `X-API-Token` ခေါင်းစီး ပါဝင်ရမည်
  `allowed_clients` ၏ အခြားတောင်းဆိုမှုအားလုံး `403 ကိုလက်ခံရရှိ
  norito_rpc_canary_denied`။
- **ga** — Norito-RPC သည် စစ်မှန်ကြောင်း အတည်ပြုထားသော ခေါ်ဆိုသူတိုင်းအတွက် ရနိုင်သည် (အဆိုအရ၊
  ပုံမှန်နှုန်းထားနှင့် ကြိုတင်စစ်ဆေးခွင့် ကန့်သတ်ချက်များ)။

အော်ပရေတာများသည် `/v1/config` မှတဆင့် ဤတန်ဖိုးများကို ဒိုင်းနမစ်ဖြင့် အပ်ဒိတ်လုပ်နိုင်ပါသည်။ အပြောင်းအလဲတိုင်း
SDKs နှင့် ကြည့်ရှုနိုင်မှုကို ခွင့်ပြုပေးသော `/rpc/capabilities` တွင် ချက်ချင်းရောင်ပြန်ဟပ်သည်။
တိုက်ရိုက်သယ်ယူပို့ဆောင်ရေးကိုယ်ဟန်အနေအထားကိုပြသရန် ဒက်ရှ်ဘုတ်များ။