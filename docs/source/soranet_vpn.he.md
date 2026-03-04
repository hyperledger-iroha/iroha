---
lang: he
direction: rtl
source: docs/source/soranet_vpn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b7e738790d81688d947760a65575f7f20e57539a93d77b37c9e84b0e41b224a
source_last_modified: "2026-01-04T17:06:57.110055+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soranet_vpn.md -->

# גשר VPN מקומי של SoraNet

גשר ה-VPN המקומי עוטף תעבורת IP לתאים קבועים של 1,024 בתים כך שלקוחות
PacketTunnel, שערי יציאה ומשטחי ממשל/חיוב משתמשים באותה מסגרת דטרמיניסטית.

- **פורמט תא:** `crates/iroha_data_model/src/soranet/vpn.rs` מקבע את ה-header
  (גרסה, class, דגלים, מזהה מעגל, flow label, seq/ack, תקציב padding,
  אורך payload) ומספק helpers ל-padding (`VpnCellV1::into_padded_frame`) ולמטעני
  control-plane/חיוב. קיבולת ה-payload היא `1024 - 42 = 982` בתים; הכותרות
  נושאות תקציב padding במילישניות.
- **Control plane:** `crates/iroha_config/src/parameters/{defaults,user,actual}.rs`
  מוסיף knobs תחת `network.soranet_vpn.*` (גודל תא, רוחב flow label, cover ratio,
  burst, heartbeat, jitter, padding budget, guard refresh, lease, מרווח DNS push,
  exit class, meter family). סיכום ה-API ללקוחות חושף את אותם שדות ל-SDKs.
- **תזמון cover:** `xtask/src/soranet_vpn.rs` בונה תוכניות cover/data דטרמיניסטיות
  מהתצורה באמצעות BLAKE3 XOF שמוזן מכל 32 הבייטים, מהדק bursts, ממסגר payloads
  עם תקציב padding מוגדר, ומייצר receipts לחיוב לפי exit class.
- **Cover ratio + seeding:** `cover_to_data_per_mille` מקבל 0–1000; ערך מפורש `0`
  מכבה cover גם כאשר `vpn.cover.enabled=true`, ותקרות burst משבצות data slots
  תוך איפוס רצף cover. `VpnBridge` גוזר seed ברירת מחדל פר-מעגל מה-circuit id
  וה-flow label (ניתן להחליף עם `set_cover_seed`).【crates/iroha_config/src/parameters/user.rs:6380】【tools/soranet-relay/src/config.rs:740】【crates/iroha_data_model/src/soranet/vpn.rs:509】【tools/soranet-relay/src/vpn_adapter.rs:224】
- **אכיפת flow-label:** `flow_label_bits` נחתך ל-1–24 ביטים (ברירת מחדל 24)
  בקלט תצורה/לקוח. בוני frame מאמתים את הרוחב המוגדר ו-helpers לפריסה דוחים
  frames שה-flow label שלהם חורג מהרוחב המותר כך ש-runtime לא יקבל labels
  גדולים בשקט.
- **אימות exit/lease:** תוויות exit class מוגבלות לרשימת allow
  `standard`/`low-latency`/`high-security` (וריאציות עם מקף/קו תחתון מתקבלות)
  ומנורמלות לפני שהן מגיעות לחוט; תוויות לא מוכרות מחזירות שגיאה בפריסת
  client/config וב-helpers של xtask. leases ב-control-plane חייבים להתאים ל-
  `u32` שניות ומיד נדחים בפריסת תצורה, סיכומי client ובוני control-plane במקום
  להיחתך בשקט.【crates/iroha_data_model/src/soranet/vpn.rs:548】【crates/iroha_config/src/parameters/user.rs:5529】【crates/iroha_config/src/client_api.rs:1549】【xtask/src/soranet_vpn.rs:42】
- **משטח לקוח:** `IrohaSwift/Sources/IrohaSwift/SoranetVpnTunnel.swift` מספק
  framer ידידותי ל-PacketTunnel שמרפד ל-1,024 בתים, אוכף את פריסת הכותרות,
  ומציע helper קטן ל-`NEPacketTunnelNetworkSettings` עבור DNS/route pushes.
  בדיקות יחידה (`IrohaSwift/Tests/IrohaSwiftTests/`) משקפות את פריסת Rust.
- **Receipt/חיוב:** שערי יציאה מפיקים `VpnSessionReceiptV1`
  (בייטים ingress/egress/cover, uptime, exit class, meter hash) באמצעות helper
  ב-`xtask/src/soranet_vpn.rs`, וכך שומרים על payloads Norito עקביים עם ברירות
  המחדל וה-meters הממשלתיים. runtime של relay מפיק receipts בעת פירוק מעגל עם
  מוני Prometheus (`soranet_vpn_session_receipts_total`,
  `soranet_vpn_receipt_*_bytes_total`) כדי שצינורות חיוב/טלמטריה יוכלו לגרד
  שימוש חי לצד בדיקות adapter/unit. מוני runtime מפרידים תעבורת data מול cover
  עבור frames/bytes (`soranet_vpn_{data,cover}_{frames,bytes}_total`), כאשר מוני
  bytes עוקבים אחרי בייטים של payload (כדי לגזור בייטים on-wire חשבו
  `frames * 1024`). תאי control/keepalive (למשל route-open control frames)
  נמדדים בנפרד דרך `soranet_vpn_control_{frames,bytes}_total` ומוחרגים ממדדי
  payload ומ-receipts.【tools/soranet-relay/src/runtime.rs:1984】【tools/soranet-relay/src/metrics.rs:744】【tools/soranet-relay/tests/vpn_adapter.rs:1】
- **Harness מדדים מקצה לקצה:** suite ה-adapter כולל כעת round-trip מתוזמן
  bridge→adapter שמזרים תאי data ו-cover על קישור duplex ומאמת מוני ingress/egress
  עבור frames/bytes של cover/data בשני הקצוות. הוא גם מאמת מסירת payload לצד
  היציאה, ומחזק את חשבונאות cover/data שהובטחה ב-SNNet-18f7 ללא הרצת runtime
  מלאה של relay.【tools/soranet-relay/tests/vpn_adapter.rs:1】
- **I/O של frames ואכיפת padding:** בוני relay משכתבים תקציבי padding מתצורה,
  אוכפים את גודל ה-frame הקבוע 1,024 בתים ואת רשימת הדגלים המותרת, ו-helpers
  אסינכרוניים לקריאה/כתיבה מפילים frames מקוצרים תוך ספירת בייטים ingress/egress.
  בדיקות overlay/adapter מגנות על padding אפס, מגבלות אורך payload ודחיית זרם
  מקוצר כדי לשמור על framing דטרמיניסטי.【tools/soranet-relay/src/vpn.rs:1】【tools/soranet-relay/tests/vpn_overlay.rs:1】【tools/soranet-relay/tests/vpn_adapter.rs:1】【xtask/src/soranet_vpn.rs:1】
- **Pacing + הזרקת cover:** `schedule_frames` מחיל `pacing_millis` כדי לשלב
  frames של cover/data שמקורם בתוכנית seeded ב-BLAKE3 (מגבלות burst/jitter),
  ו-`send_scheduled_frames` משדר בקצב המחושב עם helpers אסינכרוניים ובדיקות
  רגרסיה שמאמתות מרווחי שליחה.【tools/soranet-relay/src/vpn.rs:303】【tools/soranet-relay/tests/vpn_runtime.rs:1】
- **Runtime guard & טלמטריה:** I/O של frames, pacing והפקת receipts רצים כעת
  ב-runtime של relay בעוד חיבורי exit-bridge/control-plane נמשכים. gauge של
  Prometheus `soranet_vpn_runtime_status{state="disabled|active|stubbed"}`
  (מתויג עם `vpn_session_meter`/`vpn_byte_meter`) יחד עם מוני receipts שומרים על
  מודעות כאשר טיפול VPN פעיל לעומת stubbed/disabled.【tools/soranet-relay/src/runtime.rs:1】【tools/soranet-relay/src/config.rs:1】【tools/soranet-relay/src/metrics.rs:1】

השתמשו ב-`network.soranet_vpn` כדי לכוון תקציב heartbeat/cover לפריסות וב-
`xtask/src/soranet_vpn.rs` כדי לייצר לוחות זמנים ו-receipts לשם ראיות קבלה.

</div>
