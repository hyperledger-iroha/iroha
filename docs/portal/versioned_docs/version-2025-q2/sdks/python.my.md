---
lang: my
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2025-12-29T18:16:35.908874+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Python SDK အမြန်စတင်ပါ။

Python SDK (`iroha-python`) သည် Rust client helpers များကို ထင်ဟပ်စေပြီး သင်လုပ်နိုင်သည်
scripts၊ မှတ်စုစာအုပ်များ သို့မဟုတ် ဝဘ်နောက်ကွယ်မှ Torii နှင့် အပြန်အလှန် တုံ့ပြန်ပါ။ ဤအမြန်စတင်ပါ။
တပ်ဆင်မှု၊ ငွေပေးငွေယူ တင်ပြမှု၊ နှင့် ပွဲစဥ်ကြည့်ရှုခြင်းများ ပါဝင်ပါသည်။ နက်နဲသည်
သိုလှောင်မှုတွင် `python/iroha_python/README.md` ကို ကြည့်ရှုပါ။

## 1. ထည့်သွင်းပါ။

```bash
pip install iroha-python
```

ရွေးချယ်နိုင်သော အပိုဆောင်းများ-

- အကယ်၍ သင်သည် `pip install aiohttp` ၏ asynchronous မျိုးကွဲများကို လုပ်ဆောင်ရန် စီစဉ်ထားပါက၊
  streaming အကူအညီပေးသူများ။
- SDK ပြင်ပတွင် Ed25519 သော့ဆင်းသက်မှုကို လိုအပ်သောအခါ - `pip install pynacl`။

## 2. သုံးစွဲသူနှင့် လက်မှတ်ထိုးသူများကို ဖန်တီးပါ။

```python
from iroha_python import (
    ToriiClient,
    derive_ed25519_keypair_from_seed,
)

pair = derive_ed25519_keypair_from_seed(b"demo-seed")  # replace with secure storage
authority = pair.default_account_id("wonderland")

client = ToriiClient(
    torii_url="http://127.0.0.1:8080",
    auth_token="dev-token",  # optional: omit if Torii does not require a token
    telemetry_url="http://127.0.0.1:8080",  # optional
)
```

`ToriiClient` သည် `timeout_ms` ကဲ့သို့သော နောက်ထပ်သော့ချက်စကားလုံး အကြောင်းပြချက်များကို လက်ခံပါသည်။
`max_retries` နှင့် `tls_config`။ အကူအညီပေးသူ `resolve_torii_client_config`
Rust CLI နှင့် တူညီလိုပါက JSON configuration payload ကို ခွဲခြမ်းစိတ်ဖြာပါ။

## 3. ငွေပေးငွေယူတစ်ခု တင်သွင်းပါ။

SDK သည် သင်တည်ဆောက်ခဲသော ညွှန်ကြားချက်များကို တည်ဆောက်သူများနှင့် ငွေပေးငွေယူအကူအညီများကို ပို့ဆောင်ပေးပါသည်။
လက်ဖြင့် Norito

```python
from iroha_python import Instruction

instruction = Instruction.register_domain("research")

envelope, status = client.build_and_submit_transaction(
    chain_id="local",
    authority=authority,
    private_key=pair.private_key,
    instructions=[instruction],
    wait=True,          # poll until the transaction reaches a terminal status
    fetch_events=True,  # include intermediate pipeline events
)

print("Final status:", status)
```

`build_and_submit_transaction` သည် လက်မှတ်ရေးထိုးထားသော စာအိတ်နှင့် နောက်ဆုံးကို ပြန်ပေးသည်။
စောင့်ကြည့်ထားသော အခြေအနေ (ဥပမာ၊ `Committed`၊ `Rejected`)။ မင်းမှာ လက်မှတ်ထိုးပြီးသား
ငွေပေးငွေယူ စာအိတ် `client.submit_transaction_envelope(envelope)` သို့မဟုတ် အဆိုပါကို အသုံးပြုပါ။
JSON ဗဟိုပြု `submit_transaction_json`။

## 4. မေးမြန်းမှု အခြေအနေ

REST အဆုံးမှတ်များအားလုံးတွင် JSON အထောက်အကူများနှင့် စာရိုက်ထားသည့် ဒေတာအတန်းအစားများစွာရှိသည်။ အဘို့
ဥပမာ၊ ဒိုမိန်းများကို စာရင်းပြုစုခြင်း-

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Pagination-aware helpers (ဥပမာ၊ `list_accounts_typed`) သည် အရာဝတ္ထုတစ်ခုကို ပြန်ပေးသည်
`items` နှင့် `next_cursor` နှစ်မျိုးလုံးပါရှိသည်။

## 5. အဖြစ်အပျက်များကို တိုက်ရိုက်ကြည့်ရှုပါ။

Torii SSE အဆုံးမှတ်များကို ဂျင်နရေတာများမှတစ်ဆင့် ဖော်ထုတ်ပါသည်။ SDK သည် အလိုအလျောက် ပြန်လည်စတင်သည်။
`resume=True` နှင့် သင် `EventCursor` ကို ပေးသောအခါ။

```python
from iroha_python import PipelineEventFilterBox, EventCursor

cursor = EventCursor()

for event in client.stream_pipeline_blocks(
    status="Committed",
    resume=True,
    cursor=cursor,
    with_metadata=True,
):
    print("Block height", event.data.block.height)
```

အခြားအဆင်ပြေသည့်နည်းလမ်းများမှာ `stream_pipeline_transactions`၊
`stream_events` (ရိုက်ထည့်ထားသော filter တည်ဆောက်သူများ) နှင့် `stream_verifying_key_events`။

## 6. နောက်အဆင့်များ

- `python/iroha_python/src/iroha_python/examples/` အောက်တွင် နမူနာများကို စူးစမ်းပါ။
  အုပ်ချုပ်ရေး၊ ISO တံတားအကူများနှင့် ချိတ်ဆက်ခြင်းတို့ကို အကျုံးဝင်သော အဆုံးမှအဆုံးသို့ စီးဆင်းမှုများအတွက်။
- သင်လိုသောအခါ `create_torii_client` / `resolve_torii_client_config` ကိုသုံးပါ။
  `iroha_config` JSON ဖိုင် သို့မဟုတ် ပတ်ဝန်းကျင်မှ client ကို bootstrap လုပ်ပါ။
- Norito RPC သို့မဟုတ် Connect-specific APIs အတွက်၊ ကဲ့သို့သော အထူးပြု module များကို စစ်ဆေးပါ။
  `iroha_python.norito_rpc` နှင့် `iroha_python.connect`။

ဤအဆောက်အဦတုံးများဖြင့် သင်သည် Python မှ Torii ကို စာမရေးဘဲ လေ့ကျင့်ခန်းလုပ်နိုင်သည်။
သင်၏ကိုယ်ပိုင် HTTP ကော် သို့မဟုတ် Norito ကုဒ်ဒစ်များ။ SDK ကြီးလာသည်နှင့်အမျှ၊ ထပ်လောင်းအဆင့်မြင့်သည်။
ဆောက်လုပ်ရေးသမားများကို ပေါင်းထည့်ပါမည်။ `python/iroha_python` တွင် README ကို တိုင်ပင်ပါ။
နောက်ဆုံးအခြေအနေနှင့် ပြောင်းရွှေ့မှုမှတ်စုများအတွက် လမ်းညွှန်။