---
lang: my
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 74e29801129deccb6d5640d414289c47cf13fa9e0229fb55212b6c7710d7c5f7
source_last_modified: "2026-07-12T07:38:49.568351+00:00"
translation_last_reviewed: 2026-07-12
translator: machine-assisted
---

> ဤစာမျက်နှာသည် 2026-07-11 ရက်နေ့အထိ အကျဉ်းချုပ် ဒေသသုံးဘာသာပြန်ဖြစ်ပြီး
> စံသတ်မှတ်ချက်အပြည့်အစုံ မဟုတ်ပါ။ အတိအကျ type များ၊ API contract များနှင့်
> release လိုအပ်ချက်များအတွက် [အင်္ဂလိပ် canonical စာမျက်နှာ](bridge_proofs.md)
> ကို အသုံးပြုပါ။

# SCCP V1 တံတားအထောက်အထား — အကျဉ်းချုပ်

## ပထမ release ၏ နယ်နိမိတ်

- SCCP V1 သည် ပိတ်ထားသော surface ဖြစ်သည်။ Ethereum mainnet၊ BSC mainnet နှင့်
  TRON mainnet ကိုသာ ထောက်ပံ့ပြီး SORA ဘက်ရှိ တစ်ခုတည်းသော endpoint သည်
  `sora-taira` ဖြစ်သည်။ အခြား network profile သို့မဟုတ် SORA identity ကို
  ပယ်ချသည်။
- `SubmitBridgeProof` သည် route နှင့် ချိတ်ဆက်ထားသော typed `NativeProtocol`
  နှင့် `SccpDestination` proof များကိုသာ လက်ခံသည်။ ယေဘုယျ `Ics` နှင့်
  `TransparentZk` payload တင်သွင်းမှုကို မဖွင့်ထားဘဲ fail-closed ဖြင့် ပယ်ချသည်။

## Typed registry နှင့် မှတ်တမ်း

- `SccpRegistryV1` သည် typed နှင့် append-only ဖြစ်သည်။ lane တစ်ခုလျှင် route
  revision အများဆုံး 64 ခုနှင့် native trust anchor 4,096 ခု ထိန်းသိမ်းသည်။
  record များကို အလိုအလျောက် မဖယ်ရှားဘဲ ကန့်သတ်ချက်ကျော်သည့် နောက်ထပ် append
  ကို atomic အဖြစ် ပယ်ချသည်။
- Anchor interval သည် authenticated consensus coordinate ကို အသုံးပြုသည်။
  Ethereum တွင် finalized beacon slot၊ BSC/TRON တွင် finalized native block
  height ဖြစ်သည်။ Anchor အဟောင်းသည် successor checkpoint အပါအဝင်သာ မှန်ကန်ပြီး
  ထို့နောက် မမှန်တော့ပါ။
- Durable inbound record တွင် event/finality height နှင့်
  `anchor_interval_height` ကို သီးခြား သိမ်းသည်။ lane+anchor high-water သည်
  မြင့်တက်ရုံသာရှိပြီး successor checkpoint သည် ၎င်းထက် မနိမ့်ရပါ။ Snapshot
  hydration က index ကို အပြည့်အဝ ပြန်တွက်ပြီး missing၊ stale သို့မဟုတ် extra
  value ကို ပယ်ချသည်။ Message id ပြန်သုံးခြင်းနှင့် replay ကိုလည်း ပယ်ချသည်။

TRON source route သည် အတိအကျ
`transferToTaira(bytes,uint256,uint64 expectedNonce)` ABI ကို အသုံးပြုသည်။
အောင်မြင်စွာ လုပ်ဆောင်ရန် `expectedNonce == transferNonce` ဖြစ်ရမည်။ ထို့နောက်
storage ကို မတိုးမီ ထိုတန်ဖိုးကိုပင် canonical payload ထဲသို့ ရေးသည်။ Native
admission သည် payload recipient၊ scale လုပ်ထားသော amount နှင့် nonce တို့မှ ABI
call အပြည့်အစုံကို ပြန်လည်တည်ဆောက်သည်။ ထို့ကြောင့် ရပ်ဆိုင်းထားသော argument နှစ်ခုပါ
selector၊ stale သို့မဟုတ် future nonce နှင့် ကုန်ဆုံးသွားသော `uint64` nonce တို့ကို
အားလုံး လုံခြုံစွာ ပိတ်၍ ငြင်းပယ်သည်။

## တစ်ကြိမ်တည်း စစ်ဆေးခြင်းနှင့် deterministic limit များ

- Native နှင့် destination proof ကို canonical အဖြစ် တစ်ကြိမ်သာ decode လုပ်၍
  စျေးကြီးသော cryptographic verification ကို တစ်ကြိမ်သာ ဆောင်ရွက်သည်။ ထိုမတိုင်မီ
  consensus က conservative၊ hardware-independent work estimate ကို reserve
  လုပ်သည်။
- `[zk.sccp]` သည် proof count/bytes၊ native headers၊ Ethereum light-client
  updates၊ header bytes၊ secp256k1 recoveries၊ BLS aggregate checks/signing
  contributions နှင့် BN254 pairing-product checks အတွက် မဖြစ်မနေ သုညမဟုတ်သော
  per-proof၊ per-transaction၊ per-block limit များ သတ်မှတ်သည်။ Admission limit
  များသည် consensus-bound ဖြစ်၍ validator အားလုံးတွင် တူရမည်။

## Outbound commitment၊ retention နှင့် discovery

အောင်မြင်သော outbound message တိုင်းသည် block execution order အတိုင်း dense
`commitment_index` (`0..=511`) ရရှိသည်။ V1 ၏ fixed limit သည် block တစ်ခုလျှင်
message 512 နှင့် message တစ်ခုလျှင် canonical payload byte 4,096 ဖြစ်သည်။
`[zk.sccp]` သည် pending payload state ကို `max_pending_outbound_messages`
(default `65536`) နှင့် `max_pending_outbound_payload_bytes`
(default `268435456`) နှစ်ခုစလုံးဖြင့် ကန့်သတ်သည်။

Finality မထုတ်ပြန်မီ သို့မဟုတ် block body မဖယ်ရှားမီ Kura သည် exact canonical
header နှင့် root-authenticated SCCP archive ကို immutable အဖြစ် သိမ်းထားသည်။ Proof၊
bundle၊ proof request နှင့် recent history ပြန်တည်ဆောက်ရာတွင် historical block body
သို့မဟုတ် mutable WSV payload copy မလိုပါ။ Destination proof လက်ခံသည့်အခါ pending
payload နှင့် charge ကို atomically ဖယ်ရှားပြီး locator/index ပါ fixed terminal
descriptor ကို ထားရှိသည်။ Pending state သည် bounded ဖြစ်သော်လည်း terminal records
နှင့် immutable Kura history သည် permanent replay protection အတွက် ရည်ရွယ်ချက်ရှိစွာ
တိုးလာသည်။ `GET /v1/sccp/messages/recent` သည် `{ from, after_index }` compound
cursor ကို သုံးသည်။ Immutable evidence ကို total/operator disk usage တွင် ထည့်တွက်သော်လည်း
evictable-body budget တွင် မထည့်ပါ။

## Torii ကန့်သတ်ချက်များ

`/v1/bridge/proofs/submit` နှင့် `/v1/bridge/messages` တွင် endpoint-specific
HTTP body limit ရှိသည်။ Authentication၊ rate limit နှင့် `Content-Length` ကို
body မဖတ်မီ စစ်ဆေးပြီး chunked body ကို တင်းကျပ်သော အရွယ်အစားအထိသာ ဖတ်သည်။
အလွန်ကြီးသော request သည် `413`၊ malformed transport/JSON သည် သီးခြား `400`
ပြန်ပေးသည်။ Detached transaction payload ကို 16 MiB၊ signature payload ကို
16 KiB အထိသာ ခွင့်ပြုသည်။
