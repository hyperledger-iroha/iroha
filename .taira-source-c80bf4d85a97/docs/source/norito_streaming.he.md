<!-- Hebrew translation of docs/source/norito_streaming.md -->

---
lang: he
direction: rtl
source: docs/source/norito_streaming.md
status: complete
translator: manual
---

<div dir="rtl">

# Norito Streaming (`norito::streaming`)

המודול מספק את מבני התקשורת, ההילפרים והקודק הבסיסי של Norito Streaming Codec (NSC). הסוגים תואמים אחד-לאחד למפרט שבקובץ `norito_streaming.md` בשורש הריפו.

## מניפסטים ופריימי בקרה

`ManifestV1`, ‏`PrivacyRoute*` ופריימי הבקרה משמשים כמעטפת סריאליזציה לפאבלישר, לריליי ולצופים. אלו מבני נתונים פשוטים; שכבות גבוהות יותר אחראיות לאימות (חתימות, מדיניות כרטיסים וכו').

## קידוד ואימות סגמנטים

- `BaselineEncoder::encode_segment` בונה `EncodedSegment` עם `SegmentHeader`, ‏`ChunkDescriptor` ונתוני צ׳אנקים. הוא אוכף ID עולה, בודק גלישת טיימסטמפים ומוודא ש-`duration_ns` מקיים את הקונפיג.
- `EncodedSegment::verify_manifest` משווה את הקומיטמנטים למניפסט כדי לוודא את הצהרות הפאבלישר לפני שירות צופים.
- `BaselineDecoder::decode_segment` מאמת קומיטמנטים ובודק ש-PTS עומד ב-`timeline_start_ns + i * frame_duration_ns`, כדי למנוע דריפט.

## ניהול מפתחות

`StreamingSession` מספק הלפרים ל-handshake: `build_key_update` חותם ב-Ed25519 ומכין מטען HPKE; `process_remote_key_update` מאמת חתימות, מונים ומקבע מפתחות טרנספורט. Kyber נתמך דרך `set_kyber_remote_public` / `set_kyber_local_secret`.

`StreamingKeyMaterial` עוטף את זוג ה-Ed25519 והחומר של Kyber. `set_kyber_keys` מאמת ומאפס סוד עם drop. קונפיג: `streaming.kyber_public_key`, ‏`streaming.kyber_secret_key`, ‏`streaming.kyber_suite`, ‏`streaming.identity_*`, ‏`streaming.session_store_dir`. הערך `streaming.kyber_suite` קובע את פרופיל ה-ML-KEM והאורך הצפוי של המפתחות (`mlkem512`, ‏`mlkem768` כברירת מחדל, או `mlkem1024`; מוכרים גם השמות `kyber512/768/1024`). נכון לעכשיו ההנדשייק משתמש ב-`mlkem768` בלבד והשאר שמורים לשדרוגים עתידיים, אך ציון הערך מאפשר לתאם מראש את אורך המפתחות.

`snapshot_state`/`restore_from_snapshot` שומרים `{session_id, key_counter, sts_root}` והסוויטה, טביעות Kyber, מצב cadence ו-GCK. קבצים מוצפנים ב-ChaCha20-Poly1305.

## פריימי בקרה

`ControlFrame` מקודד `KeyUpdate`, ‏`ManifestAnnounce`, ‏`FeedbackHint`, ‏`ReceiverReport`. פריימים לא מוכרים → `ControlFrameError::UnknownVariant`.

## יכולות וניהולן

`CapabilityReport`/`CapabilityAck` מודיעים וחותמים את התוצאה. ביטים מוכרים: סוויטות HPKE, DATAGRAM, פרטיות, FEC. ביטים לא מוכרים → דחייה. `streaming.feature_bits` מגדיר את פרסומת הפאבלישר. מדיה עוברת ב-DATAGRAM כשאפשר, אחרת סטרימים חד-כיווניים עם סדר מונוטוני; פריימי בקרה נשלחים בסטרים דו-כיווני בעל קדימות גבוהה.

## מודל עומס ו-FEC

BBRv1 עם `startup_gain=2.885`, `drain_gain=1/2.885`, מחזור pacing `1.25,0.75,1...`, RTT מינימלי 5ms. הצופים שולחים `FeedbackHint` במרווחים נבחרים (Q16.16) ועוד `ReceiverReport` כל שליש. הפאבלישר מחשב עודף: `parity = clamp(ceil((loss_ewma * 1.25 + 0.005) * 12),0,6)`.

`ManifestPublisher` משתמש ב-`populate_manifest` לסמן hash יכולות, cadence ו-parity.

## טלמטריה ופרטיות

תוויות hashed (BLAKE3). מדדים: `streaming_hpke_rekeys_total`, ‏`streaming_quic_datagrams_{sent,dropped}_total`, ‏`streaming_fec_parity_current`, ‏`streaming_feedback_timeout_total`, ‏`streaming_privacy_redaction_fail_total`. היסטוגרמות ל-Encode/Decode/Network/Energy; אירועי `TelemetryEvent::{Encode,Decode,Network,Energy,Security,AuditOutcome}`. אירוע `AuditOutcome` משדר `trace_id`, גובה סלוט, סוקר, סטטוס וקישור מיתון אופציונלי עבור מסלול TRACE.

באקטים: loss (`[0,0.01)`, ...), גרדיאנט latency (`<-5`, ...), parity (`0`, `1`, `2`, `3`, `4`, `>=5`). נתוני feedback גולמיים נשמרים עד 36 שעות ומיוצאים לכל היותר פעם בדקה.

## וקטורי התאמה בין שפות

- חבילת המבחנים הרשמית של Milestone D7 נמצאת ב־`integration_tests/tests/norito_streaming_{end_to_end,feedback,fec,negative}.rs`, עם הלפרים משותפים ב־`integration_tests/tests/streaming/mod.rs`. המבחנים מריצים את ה-handshake, את התאוששות ה-FEC ואת נתיבי השגיאה בתזמון דטרמיניסטי.
- הפיקסטורות שמזינות את ההרצה זמינות ב־`integration_tests/fixtures/norito_streaming/rans/baseline.json` ובתיקיית `docs/assets/nsc/conformance/`. SDK-ים צריכים לטעון את ה-JSON ולהשוות את הגיבובים מול `docs/assets/nsc/conformance/entropy.json`.
- מדריך האינטגרציה של JavaScript מפרט את זרימת Torii Streaming ב־`docs/source/sdk/js/quickstart.md` (סעיף “Torii Queries & Streaming”). צוותי SDK אחרים אמורים להריץ את אותם וקטורים ולהגיש את אותן מדידות לצד החבילה הרשמית.

כאשר מממשים את החבילה בשפה אחרת, יש לשכפל את המניפסט ואת רצף הפריימים שמפיק `integration_tests/tests/streaming/mod.rs::baseline_test_vector_with_frames` ולהצליב את התוצאה עם החבילה שב־`docs/assets/nsc/conformance/`. ניתן להריץ `cargo test -p integration_tests --tests norito_streaming_*` כמדד ייחוס ב־CI.

</div>
