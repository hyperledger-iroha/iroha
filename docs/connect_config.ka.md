---
lang: ka
direction: ltr
source: docs/connect_config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8fa39bf0a08fb4c04d85c207bdffa287aabac979cc0496b54866b775154356e7
source_last_modified: "2026-01-05T18:22:23.392756+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Torii დაკავშირების კონფიგურაცია

Iroha Torii ავლენს არჩევით WalletConnect-ის სტილის WebSocket ბოლო წერტილებს და მინიმალურ რელეს
როდესაც ჩართულია `connect` Cargo ფუნქცია (ნაგულისხმევი). გაშვების ქცევა შეზღუდულია კონფიგურაციაზე:

- დააყენეთ `connect.enabled=false` კავშირის ყველა მარშრუტის გასათიშად (`/v2/connect/*`).
- დატოვეთ ის `true` (ნაგულისხმევი), რათა ჩართოთ WS სესიის საბოლოო წერტილები და `/v2/connect/status`.

გარემოს უგულებელყოფა (მომხმარებლის კონფიგურაცია → ფაქტობრივი კონფიგურაცია):

- `CONNECT_ENABLED` (bool; ნაგულისხმევი: `true`)
- `CONNECT_WS_MAX_SESSIONS` (გამოყენება; ნაგულისხმევი: `10000`)
- `CONNECT_WS_PER_IP_MAX_SESSIONS` (გამოყენება; ნაგულისხმევი: `10`)
- `CONNECT_WS_RATE_PER_IP_PER_MIN` (u32; ნაგულისხმევი: `120`)
- `CONNECT_FRAME_MAX_BYTES` (გამოყენება; ნაგულისხმევი: `64000`)
- `CONNECT_SESSION_BUFFER_MAX_BYTES` (გამოყენება; ნაგულისხმევი: `262144`)
- `CONNECT_PING_INTERVAL_MS` (ხანგრძლივობა; ნაგულისხმევი: `30000`)
- `CONNECT_PING_MISS_TOLERANCE` (u32; ნაგულისხმევი: `3`)
- `CONNECT_PING_MIN_INTERVAL_MS` (ხანგრძლივობა; ნაგულისხმევი: `15000`)
- `CONNECT_DEDUPE_CAP` (გამოყენება; ნაგულისხმევი: `8192`)
- `CONNECT_RELAY_ENABLED` (bool; ნაგულისხმევი: `true`)
- `CONNECT_RELAY_STRATEGY` (სტრიქონი; ნაგულისხმევი: `"broadcast"`)
- `CONNECT_P2P_TTL_HOPS` (u8; ნაგულისხმევი: `0`)

შენიშვნები:

- `CONNECT_SESSION_TTL_MS` და `CONNECT_DEDUPE_TTL_MS` იყენებენ ხანგრძლივობის ლიტერალებს მომხმარებლის კონფიგურაციაში და
  რუკა რეალურ `session_ttl` და `dedupe_ttl` ველებზე.
- `CONNECT_WS_PER_IP_MAX_SESSIONS=0` გამორთავს თითო IP სესიის ქუდი.
- `CONNECT_WS_RATE_PER_IP_PER_MIN=0` გამორთავს თითო IP ხელის ჩამორთმევის სიჩქარის შემზღუდველს.
- გულისცემის გაძლიერება ამაგრებს კონფიგურირებულ ინტერვალს ბრაუზერისთვის შესაფერის მინიმუმამდე (`ping_min_interval_ms`);
  სერვერი მოითმენს `ping_miss_tolerance` ზედიზედ გამოტოვებულ პონგებს WebSocket-ის დახურვამდე და
  ზრდის `connect.ping_miss_total` მეტრიკას.
- როდესაც გათიშულია მუშაობის დროს (`connect.enabled=false`), დაკავშირება WS და სტატუსის მარშრუტები არ არის
  დარეგისტრირდა; ითხოვს `/v2/connect/ws` და `/v2/connect/status` დაბრუნების 404.
- სერვერს სჭირდება კლიენტის მიერ მოწოდებული `sid` `/v2/connect/session`-ისთვის (base64url ან hex, 32 ბაიტი).
  ის აღარ წარმოქმნის სარეზერვო `sid`-ს.

აგრეთვე იხილე: `crates/iroha_config/src/parameters/{user,actual}.rs` და ნაგულისხმევი
`crates/iroha_config/src/parameters/defaults.rs` (მოდული `connect`).