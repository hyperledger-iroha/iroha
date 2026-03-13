---
lang: ur
direction: rtl
source: docs/connect_config.md
status: complete
translator: manual
source_hash: 15799a5698133ba1d6c5510d71d17fd60934519df890f83cb53b49d56980dc5b
source_last_modified: "2025-11-05T17:16:36.700630+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- docs/connect_config.md (Torii Connect Configuration) کا اردو ترجمہ -->

## Torii Connect کنفیگریشن

جب Cargo feature `connect` فعال ہو (جو کہ ڈیفالٹ ہے) تو Iroha Torii،
WalletConnect‑اسٹائل WebSocket endpoints اور ایک minimal in‑node relay فراہم
کرتا ہے۔ runtime رویہ (behaviour) کنفیگریشن کے ذریعے کنٹرول ہوتا ہے:

- `connect.enabled=false` سیٹ کریں تاکہ Connect کی تمام routes
  (`/v2/connect/*`) disable ہو جائیں۔
- `true` (ڈیفالٹ) رہنے دیں تاکہ WS session endpoints اور
  `/v2/connect/status` فعال رہیں۔

Environment overrides (user config → actual config):

- `CONNECT_ENABLED` (bool؛ ڈیفالٹ: `true`)
- `CONNECT_WS_MAX_SESSIONS` (`usize`; ڈیفالٹ: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (`usize`; ڈیفالٹ: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (`u32`; ڈیفالٹ: `120`)
- `CONNECT_FRAME_MAX_BYTES` (`usize`; ڈیفالٹ: `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (`usize`; ڈیفالٹ: `262144`)
- `CONNECT_PING_INTERVAL_MS` (duration؛ ڈیفالٹ: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (`u32`; ڈیفالٹ: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (duration؛ ڈیفالٹ: `15000`)
- `CONNECT_DEDUPE_CAP` (`usize`; ڈیفالٹ: `8192`)
- `CONNECT_RELAY_ENABLED` (bool؛ ڈیفالٹ: `true`)
- `CONNECT_RELAY_STRATEGY` (string؛ ڈیفالٹ: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (`u8`; ڈیفالٹ: `0`)

نوٹس:

- `CONNECT_SESSION_TTL_MS` اور `CONNECT_DEDUPE_TTL_MS`، user config میں
  duration literals استعمال کرتے ہیں اور انہیں بالترتیب مؤثر فیلڈز
  `session_ttl` اور `dedupe_ttl` پر map کیا جاتا ہے۔
- heartbeat enforcement، configured interval کو browser‑friendly کم از کم
  (`ping_min_interval_ms`) تک clamp کرتا ہے؛ سرور، پے در پے
  `ping_miss_tolerance` مرتبہ pong miss ہونے کے بعد WebSocket کنیکشن
  بند کر دیتا ہے اور میٹرک `connect.ping_miss_total` کو increment کرتا ہے۔
- جب runtime پر Connect disable ہو (`connect.enabled=false`)، تو Connect WS
  اور status routes رجسٹر ہی نہیں کیے جاتے؛ `/v2/connect/ws` اور
  `/v2/connect/status` پر آنے والی requests 404 واپس کرتی ہیں۔
- سرور `/v2/connect/session` پر client‑provided `sid` کا تقاضا کرتا ہے
  (base64url یا hex، 32 bytes)؛ fallback `sid` اب مزید generate نہیں کیا
  جاتا۔

مزید دیکھیں:
`crates/iroha_config/src/parameters/{user,actual}.rs` اور
`crates/iroha_config/src/parameters/defaults.rs` (ماڈیول `connect`) میں
defaults۔

</div>

