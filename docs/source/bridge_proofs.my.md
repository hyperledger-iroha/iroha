---
lang: my
direction: ltr
source: docs/source/bridge_proofs.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: 465d8cf704022986b169ab93133517428f8cf2ffe01a498cbda458f4a5b2e69b
source_last_modified: "2026-07-11T15:09:39+04:00"
translation_last_reviewed: 2026-07-11
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

## Torii ကန့်သတ်ချက်များ

`/v1/bridge/proofs/submit` နှင့် `/v1/bridge/messages` တွင် endpoint-specific
HTTP body limit ရှိသည်။ Authentication၊ rate limit နှင့် `Content-Length` ကို
body မဖတ်မီ စစ်ဆေးပြီး chunked body ကို တင်းကျပ်သော အရွယ်အစားအထိသာ ဖတ်သည်။
အလွန်ကြီးသော request သည် `413`၊ malformed transport/JSON သည် သီးခြား `400`
ပြန်ပေးသည်။ Detached transaction payload ကို 16 MiB၊ signature payload ကို
16 KiB အထိသာ ခွင့်ပြုသည်။
