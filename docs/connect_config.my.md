---
lang: my
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii ချိတ်ဆက်ဖွဲ့စည်းမှု

Iroha Torii သည် ရွေးချယ်နိုင်သော WalletConnect-စတိုင် WebSocket အဆုံးမှတ်များနှင့် အနည်းငယ်မျှသာ in-node relay ကို ဖော်ထုတ်ပေးသည်
`connect` Cargo အင်္ဂါရပ်ကို ဖွင့်ထားသောအခါ (မူလ)။ runtime အပြုအမူကို config တွင် ပိတ်ထားသည်-

- ချိတ်ဆက်မှုလမ်းကြောင်းများ (`/v1/connect/*`) အားလုံးကို ပိတ်ရန် `connect.enabled=false` ကို သတ်မှတ်ပါ။
- WS စက်ရှင်အဆုံးမှတ်များနှင့် `/v1/connect/status` ကိုဖွင့်ရန် ၎င်းကို `true` (မူလ) ထားလိုက်ပါ။

ပတ်ဝန်းကျင်ကို လွှမ်းမိုးထားသည် (အသုံးပြုသူ config → အမှန်တကယ် config):

- `CONNECT_ENABLED` (bool; မူရင်း- `true`)
- `CONNECT_WS_MAX_SESSIONS` (အသုံးပြုမှု; မူရင်း- `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (အသုံးပြုမှု; မူရင်း- `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (u32; မူရင်း- `120`)
- `CONNECT_FRAME_MAX_BYTES` (အသုံးပြုမှု; မူရင်း- `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (အသုံးပြုမှု; မူရင်း- `262144`)
- `CONNECT_PING_INTERVAL_MS` (ကြာချိန်၊ မူရင်း- `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (u32; မူရင်း- `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (ကြာချိန်၊ မူရင်း- `15000`)
- `CONNECT_DEDUPE_CAP` (အသုံးပြုမှု; မူရင်း- `8192`)
- `CONNECT_RELAY_ENABLED` (bool; မူရင်း- `true`)
- `CONNECT_RELAY_STRATEGY` (string; မူရင်း- `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (u8; မူရင်း- `0`)

မှတ်စုများ-

- `CONNECT_SESSION_TTL_MS` နှင့် `CONNECT_DEDUPE_TTL_MS` သည် user config တွင် ကြာချိန် literals ကိုအသုံးပြုပြီး
  အမှန်တကယ် `session_ttl` နှင့် `dedupe_ttl` အကွက်များသို့ မြေပုံ။
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` သည် per-IP session cap ကို disable လုပ်သည်။
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` သည် per-IP လက်ဆွဲနှုန်းကန့်သတ်ချက်ကို ပိတ်သည်။
- Heartbeat enforcement သည် configured interval ကို browser-friendly အနည်းဆုံး (`ping_min_interval_ms`) သို့ ကန့်သတ်ထားသည်။
  WebSocket ကို မပိတ်မီ ဆာဗာသည် `ping_miss_tolerance` ဆက်တိုက် လွတ်သွားသော pong များကို သည်းခံပေးပြီး၊
  `connect.ping_miss_total` မက်ထရစ်ကို တိုးစေသည်။
- runtime (`connect.enabled=false`) တွင် ပိတ်ထားသောအခါ၊ Connect WS နှင့် status routes များသည် မရှိပါ။
  မှတ်ပုံတင်; `/v1/connect/ws` နှင့် `/v1/connect/status` သို့ တောင်းဆိုချက်များ 404 ပြန်ပေးသည်။
- ဆာဗာသည် `/v1/connect/session` အတွက် `/v1/connect/session` (base64url သို့မဟုတ် hex၊ 32 bytes) အတွက် client-provided `sid` လိုအပ်သည်။
  ၎င်းသည် နောက်ပြန်ဆုတ် `sid` ကို မထုတ်ပေးတော့ပါ။

ကိုလည်းကြည့်ပါ- `crates/iroha_config/src/parameters/{user,actual}.rs` နှင့် ပုံသေများတွင်
`crates/iroha_config/src/parameters/defaults.rs` (module `connect`)။