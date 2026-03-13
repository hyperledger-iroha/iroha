---
lang: mn
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii Холболтын тохиргоо

Iroha Torii нь WalletConnect загварын нэмэлт WebSocket төгсгөлийн цэгүүд болон хамгийн бага зангилааны релейг харуулж байна.
`connect` ачааны функц идэвхжсэн үед (өгөгдмөл). Ажиллах үеийн зан төлөв нь тохиргоонд хаалттай байна:

- `connect.enabled=false`-г бүх Холболтын маршрутуудыг (`/v2/connect/*`) идэвхгүй болгохын тулд тохируулна уу.
- WS сессийн төгсгөлийн цэгүүд болон `/v2/connect/status`-г идэвхжүүлэхийн тулд `true` (өгөгдмөл) гэж үлдээнэ үү.

Орчны тохиргоо (хэрэглэгчийн тохиргоо → бодит тохиргоо):

- `CONNECT_ENABLED` (bool; анхдагч: `true`)
- `CONNECT_WS_MAX_SESSIONS` (ашиглах; анхдагч: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (ашиглах; анхдагч: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (u32; анхдагч: `120`)
- `CONNECT_FRAME_MAX_BYTES` (ашиглах; анхдагч: `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (ашиглах; анхдагч: `262144`)
- `CONNECT_PING_INTERVAL_MS` (хугацаа; анхдагч: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (u32; анхдагч: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (хугацаа; анхдагч: `15000`)
- `CONNECT_DEDUPE_CAP` (ашиглах; анхдагч: `8192`)
- `CONNECT_RELAY_ENABLED` (bool; анхдагч: `true`)
- `CONNECT_RELAY_STRATEGY` (мөр; анхдагч: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (u8; анхдагч: `0`)

Тэмдэглэл:

- `CONNECT_SESSION_TTL_MS` болон `CONNECT_DEDUPE_TTL_MS` нь хэрэглэгчийн тохиргоонд үргэлжлэх хугацааны литералуудыг ашигладаг.
  бодит `session_ttl` болон `dedupe_ttl` талбарууд руу зураглана.
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` нэг IP сессийн хязгаарыг идэвхгүй болгодог.
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` IP тутамд гар барих хурд хязгаарлагчийг идэвхгүй болгосон.
- Зүрхний цохилтыг хэрэгжүүлэгч нь тохируулсан интервалыг хөтчүүдэд ээлтэй хамгийн бага хэмжээнд (`ping_min_interval_ms`) тохируулдаг;
  сервер нь WebSocket-г хаахаас өмнө `ping_miss_tolerance` дараалсан алдсан теннисийг тэсвэрлэдэг.
  `connect.ping_miss_total` хэмжигдэхүүнийг нэмэгдүүлнэ.
- Ажиллаж байх үед (`connect.enabled=false`) идэвхгүй болсон үед WS болон төлөвийн чиглүүлэлтүүдийг холбох боломжгүй.
  бүртгэгдсэн; `/v2/connect/ws` болон `/v2/connect/status`-ийн хүсэлтүүд 404-ийг буцаана.
- Серверт `/v2/connect/session` (base64url эсвэл hex, 32 байт)-д зориулж үйлчлүүлэгчээс өгсөн `sid` шаардлагатай.
  Энэ нь `sid` нөөцийг үүсгэхээ больсон.

Мөн үзнэ үү: `crates/iroha_config/src/parameters/{user,actual}.rs` болон үндсэн тохиргоо
`crates/iroha_config/src/parameters/defaults.rs` (модуль `connect`).