---
lang: he
direction: rtl
source: docs/portal/docs/norito/streaming.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Norito Streaming

Norito Streaming מגדיר את פורמט ה-wire, את מסגרות הבקרה ואת הקודק הייחוסי המשמש לזרימות מדיה חיה דרך Torii ו-SoraNet. המפרט הקנוני נמצא ב-`norito_streaming.md` בשורש ה-workspace; הדף הזה מזקק את החלקים שהאופרטורים וכותבי SDK צריכים יחד עם נקודות ההגדרה.

## פורמט wire ומישור הבקרה

- **Manifests ו-frames.** `ManifestV1` ו-`PrivacyRoute*` מתארים את ציר הזמן של הסגמנטים, מתארי chunks ורמזי נתיב. מסגרות הבקרה (`KeyUpdate`, `ContentKeyUpdate` ו-feedback של cadence) נמצאות לצד ה-manifest כדי שצופים יוכלו לאמת commitments לפני הדקוד.
- **קודק בסיסי.** `BaselineEncoder`/`BaselineDecoder` כופים ids מונוטוניים של chunks, אריתמטיקת timestamps ואימות commitments. ה-hosts חייבים לקרוא ל-`EncodedSegment::verify_manifest` לפני שהם מגישים לצופים או relays.
- **Bits של feature.** משא ומתן יכולות מפרסם `streaming.feature_bits` (ברירת מחדל `0b11` = baseline feedback + privacy route provider) כדי ש-relays ולקוחות ידחו peers לא תואמים בצורה דטרמיניסטית.

## מפתחות, suites ו-cadence

- **דרישות זהות.** מסגרות הבקרה של סטרימינג נחתמות תמיד עם Ed25519. אפשר לספק מפתחות ייעודיים דרך `streaming.identity_public_key`/`streaming.identity_private_key`; אחרת נעשה שימוש חוזר בזהות הצומת.
- **HPKE suites.** `KeyUpdate` בוחר את ה-suite המשותף הנמוך ביותר; suite #1 הוא חובה (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), עם נתיב שדרוג אופציונלי ל-`Kyber1024`. בחירת ה-suite נשמרת בסשן ונבדקת בכל עדכון.
- **סבב מפתחות.** Publishers פולטים `KeyUpdate` חתום כל 64 MiB או 5 דקות. `key_counter` חייב לעלות בצורה קשיחה; רגרסיה היא שגיאה קריטית. `ContentKeyUpdate` מפיץ את Group Content Key המתגלגל, עטוף תחת ה-suite של HPKE שנוהל, ומגביל את פענוח הסגמנטים לפי ID + חלון תוקף.
- **Snapshots.** `StreamingSession::snapshot_state` ו-`restore_from_snapshot` שומרים `{session_id, key_counter, suite, sts_root, cadence state}` תחת `streaming.session_store_dir` (ברירת מחדל `./storage/streaming`). מפתחות התעבורה נגזרים מחדש בעת שחזור כדי שקריסות לא יחשפו סודות סשן.

## קונפיגורציית runtime

- **חומר מפתחות.** ספקו מפתחות ייעודיים באמצעות `streaming.identity_public_key`/`streaming.identity_private_key` (multihash Ed25519) וחומר Kyber אופציונלי דרך `streaming.kyber_public_key`/`streaming.kyber_secret_key`. כל הארבעה חייבים להיות קיימים בעת override; `streaming.kyber_suite` מקבל `mlkem512|mlkem768|mlkem1024` (כינויים `kyber512/768/1024`, ברירת מחדל `mlkem768`).
- **נתיבי SoraNet.** `streaming.soranet.*` שולט בתעבורה אנונימית: `exit_multiaddr` (ברירת מחדל `/dns/torii/udp/9443/quic`), `padding_budget_ms` (ברירת מחדל 25 ms), `access_kind` (`authenticated` מול `read-only`), `channel_salt` אופציונלי, ו-`provision_spool_dir` (ברירת מחדל `./storage/streaming/soranet_routes`).
- **Gate של sync.** `streaming.sync` מפעיל אכיפת drift עבור זרמי אודיו/וידאו: `enabled`, `observe_only`, `ewma_threshold_ms` ו-`hard_cap_ms` קובעים מתי מקטעים נדחים בגלל סטיית תזמון.

## ולידציה ו-fixtures

- הגדרות הסוג הקנוניות וה-helpers נמצאות ב-`crates/iroha_crypto/src/streaming.rs`.
- כיסוי האינטגרציה מפעיל את HPKE handshake, הפצת content-key ואת מחזור החיים של snapshots (`crates/iroha_crypto/tests/streaming_handshake.rs`). הריצו `cargo test -p iroha_crypto streaming_handshake` כדי לאמת את משטח הסטרימינג מקומית.
- לצלילה עמוקה ב-layout, בטיפול בשגיאות ובשדרוגים עתידיים, קראו את `norito_streaming.md` בשורש הריפו.
