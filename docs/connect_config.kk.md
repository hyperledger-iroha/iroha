---
lang: kk
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii қосылу конфигурациясы

Iroha Torii қосымша WalletConnect стиліндегі WebSocket соңғы нүктелерін және ең аз түйін ішіндегі релені көрсетеді
`connect` Жүк мүмкіндігі қосылғанда (әдепкі). Орындау уақытының әрекеті конфигурацияда жабылған:

- Барлық Қосылу бағыттарын (`/v1/connect/*`) өшіру үшін `connect.enabled=false` орнатыңыз.
- WS сеансының соңғы нүктелерін және `/v1/connect/status` қосу үшін оны `true` (әдепкі) қалдырыңыз.

Ортаны қайта анықтау (пайдаланушы конфигурациясы → нақты конфигурация):

- `CONNECT_ENABLED` (bool; әдепкі: `true`)
- `CONNECT_WS_MAX_SESSIONS` (пайдалану; әдепкі: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (пайдалану; әдепкі: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (u32; әдепкі: `120`)
- `CONNECT_FRAME_MAX_BYTES` (пайдалану; әдепкі: `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (пайдалану; әдепкі: `262144`)
- `CONNECT_PING_INTERVAL_MS` (ұзақтығы; әдепкі: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (u32; әдепкі: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (ұзақтығы; әдепкі: `15000`)
- `CONNECT_DEDUPE_CAP` (пайдалану; әдепкі: `8192`)
- `CONNECT_RELAY_ENABLED` (bool; әдепкі: `true`)
- `CONNECT_RELAY_STRATEGY` (жол; әдепкі: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (u8; әдепкі: `0`)

Ескертулер:

- `CONNECT_SESSION_TTL_MS` және `CONNECT_DEDUPE_TTL_MS` пайдаланушы конфигурациясында ұзақтық литералдарын пайдаланады және
  нақты `session_ttl` және `dedupe_ttl` өрістеріне салыстыру.
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` әр IP сеансының қақпағын өшіреді.
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` әр IP үшін қол алысу жылдамдығын шектегішті өшіреді.
- Жүрек соғуын күшейту конфигурацияланған аралықты браузерге қолайлы минимумға дейін қысады (`ping_min_interval_ms`);
  сервер WebSocket жабылмас бұрын `ping_miss_tolerance` дәйекті жіберіп алған теннистерге шыдайды және
  `connect.ping_miss_total` метрикасын арттырады.
- Орындау уақытында өшірілген кезде (`connect.enabled=false`), WS қосылымы және күй бағыттары қосылмайды.
  тіркелген; `/v1/connect/ws` және `/v1/connect/status` сұраулары 404 қайтарады.
- Сервер `/v1/connect/session` (base64url немесе hex, 32 байт) үшін клиент ұсынған `sid` қажет етеді.
  Ол бұдан былай резервтік `sid` жасамайды.

Сондай-ақ қараңыз: `crates/iroha_config/src/parameters/{user,actual}.rs` және әдепкі бойынша
`crates/iroha_config/src/parameters/defaults.rs` (модуль `connect`).