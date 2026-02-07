---
lang: hy
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii Connect Configuration

Iroha.
երբ `connect` Cargo ֆունկցիան միացված է (կանխադրված): Գործարկման ժամանակի վարքագիծը փակված է կազմաձևում.

- Սահմանեք `connect.enabled=false`՝ անջատելու բոլոր Connect երթուղիները (`/v1/connect/*`):
- Թողեք այն `true` (կանխադրված)՝ WS նստաշրջանի վերջնակետերը և `/v1/connect/status`-ը միացնելու համար:

Շրջակա միջավայրի անտեսում (օգտագործողի կազմաձև → իրական կազմաձև):

- `CONNECT_ENABLED` (բուլ; լռելյայն՝ `true`)
- `CONNECT_WS_MAX_SESSIONS` (օգտագործում; լռելյայն՝ `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (օգտագործում; լռելյայն՝ `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (u32; լռելյայն՝ `120`)
- `CONNECT_FRAME_MAX_BYTES` (օգտագործում; լռելյայն՝ `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (օգտագործում; լռելյայն՝ `262144`)
- `CONNECT_PING_INTERVAL_MS` (տեւողությունը; լռելյայն՝ `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (u32; լռելյայն՝ `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (տեւողությունը; լռելյայն՝ `15000`)
- `CONNECT_DEDUPE_CAP` (օգտագործում; լռելյայն՝ `8192`)
- `CONNECT_RELAY_ENABLED` (բուլ; լռելյայն՝ `true`)
- `CONNECT_RELAY_STRATEGY` (տող; լռելյայն՝ `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (u8; լռելյայն՝ `0`)

Նշումներ:

- `CONNECT_SESSION_TTL_MS`-ը և `CONNECT_DEDUPE_TTL_MS`-ը օգտագործում են տևողության բառացի բառեր օգտվողի կազմաձևում և
  քարտեզ իրական `session_ttl` և `dedupe_ttl` դաշտերին:
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0`-ն անջատում է մեկ IP նստաշրջանի գլխարկը:
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0`-ն անջատում է մեկ IP-ի համար ձեռքսեղմման արագության սահմանափակիչը:
- Սրտի զարկերի ուժեղացումը սեղմում է կազմաձևված միջակայքը բրաուզերի համար հարմար նվազագույնի (`ping_min_interval_ms`);
  սերվերը հանդուրժում է `ping_miss_tolerance` անընդմեջ բաց թողնված պոնգերը մինչև WebSocket-ը փակելը և
  ավելացնում է `connect.ping_miss_total` չափանիշը:
- Երբ գործարկման ժամանակ անջատված է (`connect.enabled=false`), Միացեք WS-ը և կարգավիճակի երթուղիները չեն
  գրանցված; հարցումները դեպի `/v1/connect/ws` և `/v1/connect/status` վերադարձնել 404:
- Սերվերը պահանջում է հաճախորդի կողմից տրամադրված `/v1/connect/session`-ի համար նախատեսված `sid` (base64url կամ hex, 32 բայթ):
  Այն այլևս չի առաջացնում հետադարձ `sid`:

Տես նաև՝ `crates/iroha_config/src/parameters/{user,actual}.rs` և լռելյայն
`crates/iroha_config/src/parameters/defaults.rs` (մոդուլ `connect`):