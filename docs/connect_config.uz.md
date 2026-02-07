---
lang: uz
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii ulanish konfiguratsiyasi

Iroha Torii ixtiyoriy WalletConnect uslubidagi WebSocket so'nggi nuqtalari va minimal tugun ichidagi o'rni ochib beradi
`connect` yuk funksiyasi yoqilganda (standart). Ish vaqtining xatti-harakati konfiguratsiya bilan bog'langan:

- Barcha Connect marshrutlarini (`/v1/connect/*`) o'chirish uchun `connect.enabled=false` sozlang.
- WS sessiyasining so'nggi nuqtalari va `/v1/connect/status` ni yoqish uchun uni `true` (standart) qoldiring.

Atrof-muhitni bekor qilish (foydalanuvchi konfiguratsiyasi → haqiqiy konfiguratsiya):

- `CONNECT_ENABLED` (bool; standart: `true`)
- `CONNECT_WS_MAX_SESSIONS` (foydalanish; standart: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (foydalanish; standart: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (u32; standart: `120`)
- `CONNECT_FRAME_MAX_BYTES` (foydalanish; standart: `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (foydalanish; standart: `262144`)
- `CONNECT_PING_INTERVAL_MS` (davomiylik; standart: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (u32; standart: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (davomiylik; standart: `15000`)
- `CONNECT_DEDUPE_CAP` (foydalanish; standart: `8192`)
- `CONNECT_RELAY_ENABLED` (bool; standart: `true`)
- `CONNECT_RELAY_STRATEGY` (satr; standart: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (u8; standart: `0`)

Eslatmalar:

- `CONNECT_SESSION_TTL_MS` va `CONNECT_DEDUPE_TTL_MS` foydalanuvchi konfiguratsiyasida davomiyligi literallaridan foydalanadi va
  haqiqiy `session_ttl` va `dedupe_ttl` maydonlariga xaritalash.
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` IP-sessiya qopqog'ini o'chiradi.
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` IP boshiga qoʻl siqish tezligini cheklovchini oʻchirib qoʻyadi.
- Yurak urishini qo'llash sozlangan intervalni brauzer uchun qulay minimal (`ping_min_interval_ms`)gacha qisqartiradi;
  server WebSocket-ni yopishdan oldin `ping_miss_tolerance` ketma-ket o'tkazib yuborilgan ponglarga toqat qiladi va
  `connect.ping_miss_total` ko'rsatkichini oshiradi.
- Ishlash vaqtida o'chirilgan bo'lsa (`connect.enabled=false`), WS va holat yo'nalishlarini ulanmaydi
  ro'yxatdan o'tgan; `/v1/connect/ws` va `/v1/connect/status` so'rovlari 404 ni qaytaradi.
- Server `/v1/connect/session` (base64url yoki hex, 32 bayt) uchun mijoz tomonidan taqdim etilgan `sid` ni talab qiladi.
  U endi zaxira `sid` hosil qilmaydi.

Shuningdek qarang: `crates/iroha_config/src/parameters/{user,actual}.rs` va standart sozlamalar
`crates/iroha_config/src/parameters/defaults.rs` (modul `connect`).