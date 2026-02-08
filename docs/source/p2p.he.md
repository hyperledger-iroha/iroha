<!-- Hebrew translation of docs/source/p2p.md -->

---
lang: he
direction: rtl
source: docs/source/p2p.md
status: complete
translator: manual
---

<div dir="rtl">

## תורי P2P ומדדים

קטע זה מסביר את קיבולות התורים ברשת peer-to-peer ואת המדדים הזמינים לניטור.

### קיבולות תור (`[network]`)

שכבת הרשת של Iroha משתמשת בערוצים חסומים כדי לשמור על שימוש זיכרון צפוי ולהציג backpressure. דגל המורשת `p2p_bounded_queues` נשמר לתאימות אך אינו משנה התנהגות. הגדירו את הקיבולות תחת `[network]`:

- `p2p_queue_cap_high` (usize, ברירת מחדל 8192)  
  - גודל התור להודעות עדיפות גבוהה ולבאפר ה-dispatch הנכנס מה-peers (קונצנזוס/בקרה).
- `p2p_queue_cap_low` (usize, ברירת מחדל 32768)  
  - גודל התור להודעות עדיפות נמוכה ולבאפר ה-dispatch הנכנס מה-peers (גוסיפ/סנכרון).
- `p2p_post_queue_cap` (usize, ברירת מחדל 2048)  
  - גודל ערוץ ה-post לכל peer (מסרים יוצאים ייעודיים).
- `p2p_subscriber_queue_cap` (usize, ברירת מחדל 8192)  
  - גודל תור המנוי הנכנס לכל מנוי שמזין את ה-node relay.

הברירות מכוילות לעומסים סביב ~20K TPS: תעבורת קונצנזוס/בקרה נשארת מגיבה בעוד לגוסיפ/סנכרון יש מרחב תמרון. התאימו לפי גודל בלוק, זמן בלוק ותנאי רשת.

הערה: תורי עדיפות גבוהה משרתים תעבורת קונצנזוס/בקרה; תורי עדיפות נמוכה מטפלים בגוסיפ ובמסלולי סנכרון. ה-relay רושם שני מנויים (גבוה/נמוך), כך שסך הבאפרים של ה-relay הוא `2 * p2p_subscriber_queue_cap` בתוספת מנויים נוספים (למשל bootstrap ג'נסיס, Torii Connect).

### הגבלת קצב לעדיפות נמוכה (`[network]`)

- `low_priority_rate_per_sec` (אופציונלי; הודעות/שנייה)  
  - מפעיל token-bucket פר-peer לתעבורת עדיפות נמוכה (גוסיפ/סנכרון) גם בכניסה וגם ביציאה. בלתי מוגדר → כבוי.
- `low_priority_burst` (אופציונלי; הודעות)  
  - קיבולת פרץ; ברירת מחדל לערך של `low_priority_rate_per_sec`.

בעת הפעלה, מסגרות נכנסות בעדיפות נמוכה (tx gossip, peer/trust gossip, health/time) נזרקות לפני ה-relay, והודעות post/שידור בעדיפות נמוכה נחסמות פר-peer. גם מסגרות בקרה של streaming מוגבלות כדי למנוע הצפה של control plane. תעבורת קונצנזוס/בקרה בעדיפות גבוהה לא מושפעת מעבר לכך.

### ריענון DNS (`[network]`)

אם `P2P_PUBLIC_ADDRESS` הוא שם מארח, ניתן לרענן חיבורים כדי לקלוט שינויי IP:

- `dns_refresh_interval_ms` (אופציונלי; בלתי מוגדר → כבוי)  
  - כאשר מוגדר, peer ינתק ויחדש חיבורים בקצב שנקבע כדי שה-resolver של המערכת יפתור מחדש. ערכים מומלצים: ‎300000–600000 (5–10 דקות) בהתאם ל-TTL ולצרכים תפעוליים.

### מדדי טלמטריה של P2P

כאשר הטלמטריה פעילה, Prometheus חושף את המדדים הבאים:

- `p2p_dropped_posts`: כמות הודעות post שנפלו בגלל תור מלא (מונוטוני).
- `p2p_dropped_broadcasts`: כמות שידורים שנפלו בגלל תור מלא.
- `p2p_subscriber_queue_full_total`: כמות הודעות נכנסות שנפלו כי תורי המנויים היו מלאים.
- `p2p_subscriber_queue_full_by_topic_total{topic="Consensus|Control|BlockSync|TxGossip|PeerGossip|Health|Other"}`: נפילות תורי מנויים לפי נושא.
- `p2p_subscriber_unrouted_total`: כמות הודעות נכנסות שנפלו כי אין מנוי תואם לנושא.
- `p2p_subscriber_unrouted_by_topic_total{topic="Consensus|Control|BlockSync|TxGossip|PeerGossip|Health|Other"}`: פירוט נפילות לפי נושא ללא מנוי תואם.
- `p2p_queue_depth{priority="High|Low"}`: עומק תור הודעות הרשת לפי עדיפות.
- `p2p_queue_dropped_total{priority="High|Low",kind="Post|Broadcast"}`: נפילות לפי עדיפות וסוג.
- `p2p_handshake_failures`: כשלי handshake (timeouts, שגיאות חתימה).
- `p2p_low_post_throttled_total`: מספר הודעות post בעדיפות נמוכה שנחסמו בטוקן-באקט.
- `p2p_low_broadcast_throttled_total`: מספר שידורים בעדיפות נמוכה שנחסמו.
- `p2p_post_overflow_total`: אירועי overflow בערוץ post פר-peer.
- `p2p_dns_refresh_total`: ריצות ריענון DNS מתוזמנות.
- `p2p_dns_ttl_refresh_total`: ריענוני DNS לפי TTL.
- `p2p_dns_resolution_fail_total`: כשלי פתרון/חיבור עבור peers מבוססי hostname.
- `p2p_dns_reconnect_success_total`: הצלחות התחברות מחדש אחרי ריענון.
- `p2p_backoff_scheduled_total`: מספר backoff שתוזמנו לפי כתובת.
- `p2p_accept_throttled_total`: חיבורים נכנסים שנדחו ע״י throttle לפי IP.
- `p2p_incoming_cap_reject_total`: דחיות בגלל `max_incoming`.
- `p2p_total_cap_reject_total`: דחיות בגלל `max_total_connections`.
- `p2p_ws_inbound_total`: חיבורי WebSocket נכנסים שאושרו.
- `p2p_ws_outbound_total`: חיבורי WebSocket יוצאים שהצליחו.

המדדים מסייעים לזהות רוויה ותקלות רשת. כל עוד התורים עומדים בקצב, מוני הנפילות נשארים 0.

דוגמת `/metrics`:

```
# HELP p2p_dropped_posts Number of p2p post messages dropped due to backpressure
# TYPE p2p_dropped_posts gauge
p2p_dropped_posts 0

# HELP p2p_subscriber_queue_full_total Number of inbound messages dropped because subscriber queues were full
# TYPE p2p_subscriber_queue_full_total gauge
p2p_subscriber_queue_full_total 3

# HELP p2p_subscriber_queue_full_by_topic_total Per-topic inbound drops caused by full subscriber queues
# TYPE p2p_subscriber_queue_full_by_topic_total gauge
p2p_subscriber_queue_full_by_topic_total{topic="Consensus"} 2

# HELP p2p_subscriber_unrouted_total Number of inbound messages dropped because no subscriber matches the topic
# TYPE p2p_subscriber_unrouted_total gauge
p2p_subscriber_unrouted_total 7

# HELP p2p_subscriber_unrouted_by_topic_total Per-topic inbound drops caused by no matching subscriber
# TYPE p2p_subscriber_unrouted_by_topic_total gauge
p2p_subscriber_unrouted_by_topic_total{topic="Consensus"} 1
...
```

### תזמון לפי נושא

- תעבורת יציאה מחולקת לנושאים לוגיים כדי למנוע head-of-line ולתעדף מסרי קונצנזוס/בקרה:
  - עדיפות גבוהה: `Consensus`, ‏`Control`
  - עדיפות נמוכה: `BlockSync`, ‏`TxGossip`, ‏`PeerGossip`, ‏`Health`, ‏`Other`
- טיפוסי מטען מיישמים `iroha_p2p::network::message::ClassifyTopic`; `iroha_core::NetworkMessage` מספק את המיפוי למסריי הליבה.
- תקציב הוגנות קטן מבטיח שגם בנוכחות תעבורת עדיפות גבוהה מתמשכת, הנושאים הנמוכים יתקדמו.

### תמיכה בפרוקסי (HTTP CONNECT / SOCKS5)

- הגדרות (`[network]`):
  - `p2p_proxy` (מחרוזת; אופציונלי): כתובת URL לפרוקסי יציאה (למשל `http://user:pass@proxy.example.com:8080`, ‏`https://proxy.example.com:8443` או `socks5://user:pass@proxy.example.com:1080`).
  - `p2p_no_proxy` (מערך מחרוזות): סיומות מארח לעקיפת הפרוקסי (למשל `.example.com`, ‏`localhost`).
  - `p2p_proxy_tls_verify` (bool; ברירת מחדל `true`): אימות hop אל פרוקסי `https://` (pinning).
  - `p2p_proxy_tls_pinned_cert_der_base64` (מחרוזת; אופציונלי): תעודת קצה (DER, base64) לשם pinning של פרוקסי `https://`. נדרש כאשר `p2p_proxy_tls_verify=true`.
- כאשר `p2p_proxy` מוגדר והיעד לא מוחרג, החייגן מבצע מנהור דרך:
  - HTTP `CONNECT host:port` עבור `http://...` / `https://...`
    - `http://...` משתמש ב־TCP לא מוצפן אל הפרוקסי.
    - `https://...` עוטף את החיבור אל הפרוקסי ב־TLS לפני `CONNECT` (דורש בנייה עם `iroha_p2p/p2p_tls`). כאשר `p2p_proxy_tls_verify=true`, נדרש pin תואם לתעודת הפרוקסי.
  - SOCKS5 `CONNECT` עבור `socks5://...` / `socks5h://...`
- שימו לב:
  - אימות בסיסי נתמך דרך `user:pass@...` בתוך כתובת ה־URL של הפרוקסי.
  - חריגים נבדקים לפי סיומת מארח.
  - השבתת `p2p_proxy_tls_verify` עלולה לחשוף פרטי התחברות לפרוקסי (ומטא-דאטה של התעבורה) ל־MITM על ה־hop אל הפרוקסי.
  - הפרוקסי חל רק על חיבורים מבוססי TCP (TCP/TLS/WS). QUIC (UDP) עוקף את הפרוקסי; אם חייבים לעבוד תמיד דרך פרוקסי, הגדירו `quic_enabled=false`.
  - בהיעדר פרוקסי, החיבור ישיר.

### מצב ריליי (Hub/Spoke/Assist)

Iroha יכולה להשתמש באופן אופציונלי ב־relay hub כדי לשפר נגישות כאשר חלק מהעמיתים נמצאים מאחורי NAT/חומות אש או ברשתות מצונזרות. זהו ריליי ברמת האפליקציה (העברת פריימים מוצפנים), ולא מנגנון ניתוב מיוחד באינטרנט.

- הגדרות (`[network]`):
  - `relay_mode` (מחרוזת; `disabled` | `hub` | `spoke` | `assist`)
    - `disabled`: ברירת מחדל; mesh ישיר בין עמיתים.
    - `hub`: מקבל חיבורים נכנסים ומעביר תעבורה בין עמיתים.
    - `spoke`: מחייג רק ל־hub ונשען על העברה (מתאים כשאין קישוריות נכנסת).
    - `assist`: שומר חיבורים ישירים כשאפשר, אך מחזיק גם חיבור ל־hub ומנתב דרך ה־hub כאשר היעד אינו מחובר ישירות.
  - `relay_hub_addresses` (מערך כתובות socket; חובה עבור `spoke` ו־`assist`)
    - כתובות ה־hub(ים) לחיוג (peers שמריצים `relay_mode=\"hub\"`). אם מסופקות מספר כתובות, הערכים נבדקים לפי הסדר והצומת יכול לבצע פייל-אובר ל־hub הבא במקרה שה־hub המועדף אינו נגיש.
  - `relay_ttl` (u8; ברירת מחדל 8)
    - מגבלת קפיצות למסגרות מועברות (מונע לולאות ריליי).

תבנית פריסה מומלצת:
- הריצו לפחות hub אחד בכתובת ציבורית יציבה (למשל node בדאטה־סנטר).
- הריצו nodes מוגבלים כ־`spoke` כך שיידרש רק חיבור יוצא ל־hub.
- הריצו validators / peers מחוברים היטב כ־`assist` כדי שיוכלו להגיע ל־spokes בלי לחייב את כל הרשת לעבוד דרך ריליי.

דוגמאות `config.toml`:

```toml
[network]
relay_mode = "hub"
```

```toml
[network]
relay_mode = "spoke"
relay_hub_addresses = ["hub.example.com:1337"]
```

```toml
[network]
relay_mode = "assist"
relay_hub_addresses = ["hub.example.com:1337"]
```

דוגמה למספר hubs (צנזורה/פייל-אובר):

```toml
[network]
relay_mode = "assist"
relay_hub_addresses = ["hub1.example.com:1337", "hub2.example.com:1337"]
```

### אסטרטגיית חיוג (Happy Eyeballs)

- Iroha מחייג מספר כתובות לכל peer (Hostname, ‏IPv6, ‏IPv4) במקביל עם השהייה קצרה, כך שהנתיב הזמין ינצח בלי להמתין לכשלי נתיבים איטיים.
- סדר העדפות ברירת מחדל: Hostname → ‏IPv6 → ‏IPv4.
- השהיית הביניים מוגדרת ע"י `happy_eyeballs_stagger_ms` (ברירת מחדל 100ms). הגדילו כדי למתן דחפים עתירי כתובות; הקטינו לפייל-אובר מהיר ברשתות איטיות.
- backoff לכל כתובת מנוהל בנפרד עם jitter מעריכי (עד 5s), המונע עומסי חיבור.

### דוגמת `[network]` TOML

```toml
[network]
# כתובת bind פנימית וכתובת מפורסמת
P2P_ADDRESS = "0.0.0.0:1337"
P2P_PUBLIC_ADDRESS = "peer1.example.com:1337"

# קיבולות תורים חסומים
p2p_queue_cap_high = 8192     # consensus/control
p2p_queue_cap_low  = 32768    # gossip/sync
p2p_post_queue_cap = 2048     # per-peer post channel
p2p_subscriber_queue_cap = 8192  # inbound relay subscriber queue

# פרמטרים נוספים (להמחשה)
block_gossip_size = 4        # תקרת פאנאאוט ל־block sync והצבעות זמינות (דגימת עמיתים + עדכוני block sync)
block_gossip_period_ms = 10000
block_gossip_max_period_ms = 30000
peer_gossip_period_ms = 1000
peer_gossip_max_period_ms = 30000
transaction_gossip_size = 500
transaction_gossip_resend_ticks = 3
```

- מרווחי gossip/idle מוגבלים למינימום של 100ms כדי למנוע לולאות סחרור.
- גוסיפ כתובות עמיתים מונע ע״י שינוי עם backoff מעריכי עד `peer_gossip_max_period_ms`
  (ומוגבל כאשר ה־relay מפיל מסרים נכנסים); דגימת block sync מבצעת backoff דומה עד
  `block_gossip_max_period_ms` כאשר אין התקדמות.
- גוסיפ טרנזקציות נעצר זמנית כאשר relay backpressure פעיל (נפילות בתור subscriber)
  וחוזר אחרי `transaction_gossip_period_ms * transaction_gossip_resend_ticks` כדי למנוע הצפה.
</div>
