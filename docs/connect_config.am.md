---
lang: am
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii የግንኙነት ውቅር

Iroha I18NT0000002X አማራጭ የWalletConnect-style WebSocket የመጨረሻ ነጥቦችን እና አነስተኛ የመስቀለኛ መንገድ ቅብብል ያጋልጣል
የ `connect` ጭነት ባህሪ ሲነቃ (ነባሪ)። የሩጫ ጊዜ ባህሪው በማዋቀር ላይ ተዘግቷል፡-

- ሁሉንም የግንኙነት መንገዶችን (`/v1/connect/*`) ለማሰናከል `connect.enabled=false` ያዘጋጁ።
- የ WS ክፍለ ጊዜ የመጨረሻ ነጥቦችን እና `/v1/connect/status` ለማንቃት `true` (ነባሪ) ይተዉት።

አካባቢ ይሽራል (የተጠቃሚ ውቅር → ትክክለኛው ውቅር)

- `CONNECT_ENABLED` (ቦል፣ ነባሪ፡ `true`)
- `CONNECT_WS_MAX_SESSIONS` (ተጠቀም፣ ነባሪ፡ `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (ተጠቀም፣ ነባሪ፡ `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (u32፤ ነባሪ፡ `120`)
- `CONNECT_FRAME_MAX_BYTES` (ተጠቀም፣ ነባሪ፡ `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (ተጠቀም፣ ነባሪ፡ `262144`)
- `CONNECT_PING_INTERVAL_MS` (የቆይታ ጊዜ፤ ነባሪ፡ `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (u32፤ ነባሪ፡ `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (የቆይታ ጊዜ፤ ነባሪ፡ `15000`)
- `CONNECT_DEDUPE_CAP` (ተጠቀም፣ ነባሪ፡ `8192`)
- `CONNECT_RELAY_ENABLED` (ቦል፣ ነባሪ፡ `true`)
- `CONNECT_RELAY_STRATEGY` (ሕብረቁምፊ፡ ነባሪ፡ `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (u8፤ ነባሪ፡ `0`)

ማስታወሻዎች፡-

- `CONNECT_SESSION_TTL_MS` እና `CONNECT_DEDUPE_TTL_MS` በተጠቃሚ ውቅር እና ቆይታ ላይ ቃል በቃል ይጠቀማሉ።
  ካርታ ወደ ትክክለኛው `session_ttl` እና I18NI0000037X መስኮች።
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` በየአይ ፒ ክፍለ ጊዜ ቆብ ያሰናክላል።
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` በየአይ ፒ የእጅ መጨባበጥ መጠን ገዳዩን ያሰናክላል።
- የልብ ምት ማስፈጸሚያ የተዋቀረውን የጊዜ ክፍተት ለአሳሽ ተስማሚ ዝቅተኛው (`ping_min_interval_ms`) ያቆማል።
  አገልጋዩ ዌብሶኬትን ከመዝጋቱ በፊት I18NI0000041X ተከታታይ ያመለጡ ፓንጎችን ይታገሣል።
  የ `connect.ping_miss_total` መለኪያን ይጨምራል።
- በሚሠራበት ጊዜ (`connect.enabled=false`) ሲሰናከል WS አገናኝ እና የሁኔታ መስመሮች አይደሉም
  የተመዘገበ; ወደ `/v1/connect/ws` እና I18NI0000045X መመለሻ 404 ጥያቄዎች.
- አገልጋዩ በደንበኛ የቀረበ `sid` ለI18NI0000047X (base64url ወይም hex፣ 32 bytes) ይፈልጋል።
  ከአሁን በኋላ ውድቀትን `sid` አያመነጭም።

በተጨማሪ ይመልከቱ፡ `crates/iroha_config/src/parameters/{user,actual}.rs` እና ነባሪዎች በ ውስጥ
`crates/iroha_config/src/parameters/defaults.rs` (ሞዱል I18NI0000051X)።