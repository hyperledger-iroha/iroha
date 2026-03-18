<!-- Hebrew translation of docs/source/norito_streaming_transport_design.md -->

---
lang: he
direction: rtl
source: docs/source/norito_streaming_transport_design.md
status: complete
translator: manual
---

<div dir="rtl">

# תכנון שכבת הטרנספורט והבקרה של Norito Streaming

## מטרה

מסמך זה מאחד את בחירות הטרנספורט, הקריפטוגרפיה והניטור הדרושות למימוש אבני הדרך D2–D7 ברודמאפ (Norito Streaming). הוא אינו מחליף את המפרט הנורמטיבי ב-`norito_streaming.md`, אלא מעגן את ההחלטות שעל המיישמים לאמץ בעת חיבור שכבת הבקרה, QUIC והטלמטריה בקוד Rust.

## תחום הכיסוי

- בחירת פרופיל HPKE, חיי מפתח ופריימינג ח handshake ב-`StreamingSession`.
- ניהול יכולות QUIC (זרמים, DATAGRAM, קדימויות ופולבקים דטרמיניסטיים).
- משוואות, טיימרים ואינווריאנטים של FEC שמבטיחים החלטות עודפות אחידות בכל חומרה.
- כללי טלמטריה השומרים על פרטיות טננט תוך ספקת המדדים הדרושים לכלי הניטור ואבני הדרך.

הבחירות מחייבות את `iroha_p2p`, ‏`iroha_crypto::streaming`, ‏`norito::streaming` ואת התיעוד התפעולי שנשלח במיילסטונים D3–D7.

## HPKE – בחירות פרופיל

### יעדים

- לספק סודיות קדמית פוסט-קוונטית מבלי לוותר על דטרמיניזם.
- להישאר תואמים לפורמט על החוט של `StreamingSession::snapshot_state`.
- להימנע ממשא ומתן על סוויטות במסלול הקריטי.

### סוויטות נבחרות

פאבלישרים וצופים **חייבים** לממש שתי סוויטות HPKE בסדר הבא:

1. `AuthPsk` + `Kyber768` + `HKDF-SHA3-256` + `ChaCha20Poly1305`
2. `AuthPsk` + `Kyber1024` + `HKDF-SHA3-256` + `ChaCha20Poly1305`

סוויטה #1 היא חובה לשימוש; #2 שמורה לחומרה עתידית. שימוש בסוויטה #2 מותר רק אם שני הצדדים מצהירים על `streaming_capabilities.hpke_kyber1024 = true`, אחרת חובה לחזור לסוויטה #1.

### PSK

ה-PSK נגזר דטרמיניסטית מה-commitment hash של המניפסט:

```
psk = BLAKE3("nsc-hpke-psk" || manifest.commitment_hash)
psk_id = manifest.commitment_hash[0..15]
```

`psk_id` משודר בפריים הבקרה כך שהצופה יוודא שסיפק את אותה מחרוזת; אי התאמה גוררת `StreamingProtocolError::PskMismatch`.

### רוטציה ומונים

- `key_counter` עולה עם כל `KeyUpdate` חתום; סיבוב מפתח כל 64MiB או 5 דקות (הקודם מביניהם).
- הפאבלישר מייצר זוג מפתחות HPKE חדש ומקודד אותו ב-`HpkePayload`.
- הצופה דוחה `KeyUpdate` עם מונה שאינו גדול יותר מהמונו האחרון.
- הסשן מאחסן את מזהה הסוויטה (`0x0001` / `0x0002`) כדי שכל שחזור ייעשה באותה בחירה.

### הערות דטרמיניזם

- קפסולציה של Kyber משתמשת בגרסה הדטרמיניסטית של FIPS 203 §8.5 עם coins שמגיעים מ-`BLAKE3("nsc-kyber-coins" || session_id || key_counter || role)`.
- nonce של ChaCha20Poly1305 נגזר ממספרי הרצף של QUIC DATAGRAM ומסולט משותף מהקשר HPKE, להבטחת AEAD זהה.

## ניהול יכולות QUIC

### סטרים בקרה

- סטרים `0x0` משדר `TransportCapabilities` מוצפן ב-Norito במהלך ה-handshake.
- השדות כוללים `hpke_suite_mask`, דגל `supports_datagram`, גודל DATAGRAM מרבי, פרק הזמן של פידבק FEC וגרנולריות פרטיות.
- כל כיוון שולח פריים אחד בלבד; הפאבלישר מקדים אותו לכל מדיה.

### פתרון יכולות

1. שני הצדדים מנתחים את הפריים.
2. בוחרים את הסוויטה בעלת הביט המשותף הנמוך ביותר; אין חפיפה → `TransportError::MissingHpkeSuite`.
3. `supports_datagram` תקף רק אם שני הצדדים true.
4. `max_segment_datagram_size` = מינימום. הפאבלישר מפצל בהתאם.
5. `fec_feedback_interval_ms` = מקסימום. קצב אחיד שומר על תקציב.

### שימוש בזרמים ודאטגרמים

- עם DATAGRAM: חלונות סגמנטים ממופים לרצף רציף; אין דילוגי מספרים, ובמידת הצורך מתווספים DATAGRAM מרופדים.
- ללא DATAGRAM: זרם חד-כיווני יחיד לכל חלון, עם כותרת דטרמיניסטית `chunk_id/is_parity/payload_len`.
- הודעות בקרה נשלחות על סטרים דו-כיווני בעל קדימות 256 (מדיה 128, ארכיון 64).

### גיבוש האש

הטופס `(suite_id, datagram, max_dgram, fec_interval)` מוזן ל-
`BLAKE3("nsc-transport-capabilities" || ...)` וה-hash נשמר במניפסט.

## פידבק FEC דטרמיניסטי

### הודעות

- הצופה שולח `FeedbackHint` כל `fec_feedback_interval_ms`, עם `(loss_ewma, latency_gradient, observed_rtt_ms)`.
- כל שלישית הודעות מתווסף `ReceiverReport` עם `delivered_sequence`, ‏`parity_applied`, ‏`fec_budget`.
- הפאבלישר שומר את המידע ב-`feedback_state` ומבטיח הופעתו במניפסט.
- למוד שיחות בזמן אמת, ISI של `Kaigi` מנהלים חדרים ונשמרים במטא-דאטה דומייני.

### חישוב הפסדים

- EWMA עם `alpha = 0.2`, חישוב ב-Q16.16:

```
loss_ewma_fp = loss_ewma_fp + ((sample_fp - loss_ewma_fp) * 13107) >> 16
```

### משוואת עודפות

```
parity = clamp(ceil((loss_ewma * 1.25 + 0.005) * 12), 0, 6)
```

- משתמשים בחשבון Q16.16 וב-ceil דטרמיניסטי; שינוי ברמת העודפות מדווח לטלמטריה. הערך אינו קטן מהערך הקודם.

### טיימרים

- טיימר פידבק מתיישר עם תחילת הסשן. רזולוציה מינימלית 10ms והמרווח כפולה של ערך זה. איחור → תזמון מחדש לאותו יישור.

## טלמטריה ופרטיות

- נתונים נאספים בבאקטים של `privacy_bucket_granularity` (ברירת מחדל 30 שניות).
- תוויות: `session_id` + באקט במקום מזהה סטרים; עד 1024 סשנים ביום.
- ממוצעים מעוגלים כלפי מטה (למשל RTT ל-5ms הקרוב).
- `StreamingTelemetryEvent` אינו נושא `tenant_hash`; Norito `TenantTelemetry` כולל רק מזהה מגובב ובאקט.

## קונפיגורציה

- `hpke_suite_mask`, ‏`supports_datagram`, ‏`max_segment_datagram_size`, ‏`fec_feedback_interval_ms`, ‏`privacy_bucket_granularity_ms` ב-`StreamingConfig`.
- CLI `iroha_cli streaming capabilities` מציג את המצב.
- המניפסט מכיל `transport_capabilities_hash`; `/v1/streaming/manifest` מאפשר אימות חיצוני.

## רפרנסים

- `iroha_crypto::streaming/hpke.rs`
- `iroha_p2p::streaming::quic`
- `norito::streaming`
- `docs/source/norito_streaming.md`

</div>
