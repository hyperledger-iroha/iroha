---
lang: my
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI လက်မှတ်ထိုး
---------------

- Torii OpenAPI spec (`torii.json`) ကို လက်မှတ်ရေးထိုးရမည်ဖြစ်ပြီး မန်နီးဖက်စ်ကို `cargo xtask openapi-verify` မှ အတည်ပြုပါသည်။
- ခွင့်ပြုထားသော လက်မှတ်ထိုးသူသော့များသည် `allowed_signers.json` တွင် အသက်ရှင်နေပါသည်။ လက်မှတ်ထိုးသောကီး ပြောင်းလဲသည့်အခါတိုင်း ဤဖိုင်ကို လှည့်ပါ။ `version` အကွက်ကို `1` တွင်ထားပါ။
- CI (`ci/check_openapi_spec.sh`) သည် နောက်ဆုံးနှင့် လက်ရှိ specs နှစ်ခုလုံးအတွက် ခွင့်ပြုစာရင်းကို ပြဌာန်းထားပြီးဖြစ်သည်။ အခြားပေါ်တယ် သို့မဟုတ် ပိုက်လိုင်းသည် လက်မှတ်ရေးထိုးထားသော spec ကို သုံးစွဲပါက၊ ပျံ့လွင့်မှုမဖြစ်စေရန် ၎င်း၏အတည်ပြုချက်အဆင့်ကို တူညီသောခွင့်ပြုစာရင်းဖိုင်တွင် ညွှန်ပြပါ။
- သော့လှည့်ပြီးနောက် ပြန်လည်လက်မှတ်ထိုးရန်-
  1. `allowed_signers.json` ကို အများသူငှာသော့အသစ်ဖြင့် အပ်ဒိတ်လုပ်ပါ။
  2. သတ်မှတ်ချက်- `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>` ကို ပြန်ထုတ်/လက်မှတ်ထိုးပါ။
  3. မန်နီးဖက်စ်သည် ခွင့်ပြုစာရင်းနှင့် ကိုက်ညီကြောင်း အတည်ပြုရန် `ci/check_openapi_spec.sh` (သို့မဟုတ် `cargo xtask openapi-verify` ကို ကိုယ်တိုင်) ပြန်လည်လုပ်ဆောင်ပါ။