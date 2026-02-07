---
lang: az
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii Qoşulma Konfiqurasiyası

Iroha Torii isteğe bağlı WalletConnect tipli WebSocket son nöqtələrini və minimal node relesini ifşa edir
`connect` Yük funksiyası aktiv olduqda (standart). İş vaxtı davranışı konfiqurasiyada qapalıdır:

- Bütün Qoşulma marşrutlarını (`/v1/connect/*`) söndürmək üçün `connect.enabled=false` təyin edin.
- WS sessiyasının son nöqtələrini və `/v1/connect/status`-ni aktivləşdirmək üçün onu `true` (defolt) buraxın.

Ətraf mühitin ləğvi (istifadəçi konfiqurasiyası → faktiki konfiqurasiya):

- `CONNECT_ENABLED` (bool; default: `true`)
- `CONNECT_WS_MAX_SESSIONS` (istifadə; standart: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (istifadə; standart: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (u32; defolt: `120`)
- `CONNECT_FRAME_MAX_BYTES` (istifadə; standart: `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (istifadə; standart: `262144`)
- `CONNECT_PING_INTERVAL_MS` (müddət; standart: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (u32; defolt: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (müddət; standart: `15000`)
- `CONNECT_DEDUPE_CAP` (istifadə; standart: `8192`)
- `CONNECT_RELAY_ENABLED` (bool; default: `true`)
- `CONNECT_RELAY_STRATEGY` (string; default: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (u8; defolt: `0`)

Qeydlər:

- `CONNECT_SESSION_TTL_MS` və `CONNECT_DEDUPE_TTL_MS` istifadəçi konfiqurasiyasında müddət literallarından istifadə edir və
  faktiki `session_ttl` və `dedupe_ttl` sahələrinə xəritə.
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` IP başına sessiya qapağını deaktiv edir.
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` IP başına əl sıxma sürəti məhdudlaşdırıcısını söndürür.
- Ürək döyüntüsünün tətbiqi konfiqurasiya edilmiş intervalı brauzer üçün əlverişli minimuma (`ping_min_interval_ms`) uyğunlaşdırır;
  server WebSocket-i bağlamadan əvvəl `ping_miss_tolerance` ardıcıl buraxılmış tennislərə dözür və
  `connect.ping_miss_total` metrikasını artırır.
- İş vaxtında söndürüldükdə (`connect.enabled=false`), WS-ə qoşulun və status marşrutları deyil
  qeydiyyatdan keçmiş; `/v1/connect/ws` və `/v1/connect/status` sorğuları 404-ü qaytarır.
- Server `/v1/connect/session` (base64url və ya hex, 32 bayt) üçün müştəri tərəfindən təmin edilmiş `sid` tələb edir.
  O, artıq `sid` ehtiyatını yaratmır.

Həmçinin baxın: `crates/iroha_config/src/parameters/{user,actual}.rs` və defoltlar
`crates/iroha_config/src/parameters/defaults.rs` (modul `connect`).