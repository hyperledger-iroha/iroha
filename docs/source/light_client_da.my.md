---
lang: my
direction: ltr
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2025-12-29T18:16:35.975661+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Light Client ဒေတာရရှိနိုင်မှုနမူနာ

Light Client Sampling API သည် စစ်မှန်ကြောင်း အတည်ပြုထားသော အော်ပရေတာများကို ပြန်လည်ရယူရန် ခွင့်ပြုသည်။
လေယာဉ်ပျံသန်းမှုပိတ်ဆို့ခြင်းအတွက် Merkle-စစ်မှန်ကြောင်းအထောက်အထားပြထားသော RBC အတုံးနမူနာများ။ အလင်းဖောက်သည်။
ကျပန်းနမူနာတောင်းဆိုမှုများကို ထုတ်ပေးနိုင်ပြီး၊ ဆန့်ကျင်ဘက် ပြန်ပေးထားသည့် အထောက်အထားများကို စစ်ဆေးနိုင်သည်။
ကြော်ငြာထားသော အတုံးအခဲများကို အမြစ်မစွဲဘဲ ဒေတာရရှိနိုင်ကြောင်း ယုံကြည်မှုတည်ဆောက်ပါ။
payload တစ်ခုလုံးကို ရယူခြင်း။

## အဆုံးမှတ်

```
POST /v1/sumeragi/rbc/sample
```

အဆုံးမှတ်သည် ပြင်ဆင်သတ်မှတ်ထားသော တစ်ခုနှင့် ကိုက်ညီသော `X-API-Token` ခေါင်းစီး လိုအပ်သည်
Torii API တိုကင်များ တောင်းဆိုမှုများသည် ထပ်လောင်းအတိုးနှုန်း ကန့်သတ်ထားပြီး နေ့စဉ်နှင့်အမျှ အကျုံးဝင်ပါသည်။
ခေါ်ဆိုသူတိုင်း ဘိုက်ဘတ်ဂျက်၊ တစ်ခုခုကိုကျော်လွန်ပါက HTTP 429 ကို ပြန်ပေးသည်။

### တောင်းခံလွှာ

```json
{
  "block_hash": "<hex-encoded block hash>",
  "height": 42,
  "view": 0,
  "count": 3,
  "seed": 12345
}
```

* `block_hash` - hex ရှိ ပစ်မှတ်ဘလောက် ဟက်ရှ်။
* `height`၊ `view` - RBC စက်ရှင်အတွက် tuple ကို ခွဲခြားသတ်မှတ်ခြင်း။
* `count` - လိုချင်သောနမူနာအရေအတွက် (ပုံသေ ၁ သို့၊ ဖွဲ့စည်းမှုဖြင့် ကန့်သတ်ထားသည်)။
* `seed` - မျိုးပွားနိုင်သောနမူနာအတွက် စိတ်ကြိုက်သတ်မှတ်နိုင်သော RNG မျိုးစေ့။

### တုံ့ပြန်မှုကိုယ်ထည်

```json
{
  "block_hash": "…",
  "height": 42,
  "view": 0,
  "total_chunks": 128,
  "chunk_root": "…",
  "payload_hash": "…",
  "samples": [
    {
      "index": 7,
      "chunk_hex": "…",
      "digest_hex": "…",
      "proof": {
        "leaf_index": 7,
        "depth": 8,
        "audit_path": ["…", null, "…"]
      }
    }
  ]
}
```

နမူနာထည့်သွင်းမှုတစ်ခုစီတွင် အတုံးအခဲအညွှန်း၊ payload bytes (hex)၊ SHA-256 အရွက်ပါရှိသည်။
အချေအတင်၊ နှင့် Merkle ပါဝင်မှု အထောက်အထား (ရွေးချယ်နိုင်သော မွေးချင်းများ hex အဖြစ် ကုဒ်လုပ်ထားသော
ကြိုးများ)။ ဖောက်သည်များသည် `chunk_root` အကွက်ကို အသုံးပြု၍ အထောက်အထားများကို စစ်ဆေးနိုင်သည်။

## အကန့်အသတ်နှင့် ဘတ်ဂျက်

* ** တောင်းဆိုမှုတစ်ခုအတွက် အများဆုံးနမူနာများ** – `torii.rbc_sampling.max_samples_per_request` မှတစ်ဆင့် ပြင်ဆင်သတ်မှတ်နိုင်သည်။
* ** တောင်းဆိုမှုတစ်ခုအတွက် အများဆုံး bytes** – `torii.rbc_sampling.max_bytes_per_request` ကို အသုံးပြု၍ ပြဋ္ဌာန်းထားသည်။
* **နေ့စဉ် ဘိုက်ဘတ်ဂျက်** – `torii.rbc_sampling.daily_byte_budget` မှတစ်ဆင့် ခေါ်ဆိုသူတစ်ဦးစီကို ခြေရာခံထားသည်။
* **နှုန်းထားကန့်သတ်ချက်** – သီးခြားတိုကင်ပုံး (`torii.rbc_sampling.rate_per_minute`) ကို အသုံးပြု၍ ပြဋ္ဌာန်းထားသည်။

မည်သည့်ကန့်သတ်ချက်ထက်ကျော်လွန်တောင်းဆိုမှုများ HTTP 429 (CapacityLimit) ကို ပြန်ပေးသည်။ ရိုက်မယ်
စတိုးကို မရရှိနိုင်ပါ သို့မဟုတ် စက်ရှင်သည် အဆုံးမှတ်၏ payload bytes ပျောက်ဆုံးနေပါသည်။
HTTP 404 ကို ပြန်ပေးသည်။

## SDK ပေါင်းစည်းခြင်း။

### JavaScript

`@iroha/iroha-js` သည် `ToriiClient.sampleRbcChunks` အကူအညီပေးသူ ဒေတာကို ထုတ်ပြသည်
ရရှိနိုင်မှုစစ်ဆေးသူများသည် ၎င်းတို့၏ကိုယ်ပိုင်ထုတ်ယူမှုကို မပြုလုပ်ဘဲ အဆုံးမှတ်ကို ခေါ်ဆိုနိုင်သည်။
ယုတ္တိဗေဒ။ အကူအညီပေးသူက hex payloads များကို validate လုပ်ပေးပြီး၊ integers များကို normalizes လုပ်ပြီး returns များကို ပြန်ပေးသည်။
အထက်ဖော်ပြပါ တုံ့ပြန်မှုအစီအစဉ်ကို ထင်ဟပ်နေသည့် အရာဝတ္ထုများကို ရိုက်ထည့်သည်-

```js
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL, {
  apiToken: process.env.TORII_API_TOKEN,
});

const sample = await torii.sampleRbcChunks({
  blockHash: "3d...ff",
  height: 42,
  view: 0,
  count: 3,
  seed: Date.now(),
});

if (!sample) {
  throw new Error("RBC session is not available yet");
}

for (const { digestHex, proof } of sample.samples) {
  verifyMerklizedChunk(sample.chunkRoot, digestHex, proof);
}
```

JS-04 တူညီမှုကို ကူညီပေးသည့် ဆာဗာသည် ပုံပျက်နေသော ဒေတာကို ပြန်ပေးသည့်အခါ အထောက်အကူပေးသူက ပစ်သည်။
စမ်းသပ်မှုများသည် Rust နှင့် Python SDK များနှင့်အတူ ဆုတ်ယုတ်မှုကို ထောက်လှမ်းသည်။ သံချေး
(`iroha_client::ToriiClient::sample_rbc_chunks`) နှင့် Python
(`IrohaToriiClient.sample_rbc_chunks`) သင်္ဘောနှင့်ညီမျှသော အကူအညီပေးသူများ၊ ဘယ်ဟာကိုသုံးပါ။
သင်၏နမူနာကြိုးကြိုးနှင့် ကိုက်ညီသည်။