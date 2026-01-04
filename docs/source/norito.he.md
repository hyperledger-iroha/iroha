---
lang: he
direction: rtl
source: norito.md
status: needs-update
generator: scripts/sync_docs_i18n.py
---

% פרטי מימוש Norito

> NOTE: This translation has not yet been updated for the v1 layout defaults (flags=0x00 by default; explicit header flags are allowed) and header-only decoding. Refer to `norito.md` for current semantics.

<div dir="rtl">

# Norito: פרטי מימוש הסריאליזציה

Norito הוא קודק הסריאליזציה הבינארי של Iroha. הוא מגדיר כיצד להפוך ערכים לבייטים עבור הודעות על־חוט, אחסון בדיסק ו-API בין רכיבים, וכיצד לאמת ולפענח את אותם בייטים חזרה לערכים.

## למה Norito?

- **קונסנזוס דטרמיניסטי** – כל צומת מפיק בדיוק אותם בייטים לאותו נתון, ללא תלות בחומרה.
- **שלמות וסכמה** – כותרת קומפקטית נושאת hash של הסכמה ו-CRC64 לבדיקת תקינות לפני הפענוח.
- **יעילות ביצועים וגודל** – פריסות מתועדות (Packed Struct/Seq, אורכים קומפקטיים) עם תאימות לאחור דרך דגלי תכונה.
- **איחוד across workspace** – אותו קודק (והתנהגות שגיאות) לכל שכבות הפרויקט: מודל נתונים, IVM/Kotodama, CLI ורשת.

> **הערה חשובה:** Norito איננו SCALE. אין שימוש ב-`parity-scale-codec`; האזכורים קיימים רק להשוואה.

## יעדים והיקף

- **דטרמיניסטי:** קידוד/פענוח זהים בכל פלטפורמה.
- **יעיל:** פורמטים קומפקטיים, גישה צפויה לזיכרון ודחיסה אופציונלית.
- **בטיחות כברירת מחדל:** כותרת/Checksum מאומתים, שגיאות מובְנות, Strict-Safe אופציונלי.
- **הרחבה:** דגל כותרת אחד מנהל פיצ׳רים כדי לשמור על תאימות.

## מדיניות פיתוח ו-CI

- מאוכף איסור על `serde`/`serde_json` בקוד Production. השתמשו ב-`norito::json` וב-`norito::{Encode, Decode}`. ראו `.github/workflows/serde-guard.yml`, ‎`scripts/deny_serde_json.sh`, `scripts/check_no_direct_serde.sh`.

## תג Merkle לשדות Shielded (ZK)

- עלים: `Hash(b"iroha:zk:shield:cm:v1\0" || cm)` (Blake2b-32). הורים: `Hash(left || right)`.
- שורש עץ ריק עומק d: `R_0 = Hash(tag || 0^32)`, ‎`R_{i+1} = Hash(R_i || R_i)`.
- פונקציות: `shielded_leaf_from_commitment`,‏ `shielded_empty_root` ב-`iroha_crypto::MerkleTree<[u8;32]>`.

## מעטפת Payload מוצפן סודי

- `ConfidentialEncryptedPayload`: בייט גרסת פרוטוקול + Norito Struct.
- גרסה `0x01`: `version | ephemeral_pubkey:[32] | nonce:[24] | ciphertext:Vec<u8>`.
- ה-ciphertext מכיל XChaCha20-Poly1305 כולל תג Poly1305.
- ארנקים מפיקים מפתח X25519 אפמרלי, חשבון משותף, מצפינים עם nonce באורך 24.
- מטענים ריקים (בבדיקות) מותר אך בייצור חובה למלא הכל.

## מבנה הקרייטים ותכונות

- `crates/norito`: ליבת הקודק, כותרות, CRC64, דחיסה, Packed Layouts.
- `crates/norito_derive`: מאקרו derive, מיובא תחת `norito` עם הפיצ׳ר `derive`.
- תכונות ברירת מחדל: `packed-seq`,‏ `packed-struct`,‏ `compact-len`,‏ `compression`,‏ `gpu-compression`,‏ `schema-structural`.

## פורמט הכותרת (`core::Header`)

- 4 בייט Magic: `NRT0`.
- גרסאות Major/M Minor.
- דגלי Layout (בייט).
- checksum CRC64-ECMA.
- אורכי Payload (לפני דחיסה).
- hash סכמה (בייטים).

## קודק Bare

- `norito::codec::{Encode, Decode}`:
  - `to_bytes<T>(&T)` → Payload ללא כותרת.
  - `decode_from_slice` / `decode_bare`.
- כשצריך כותרת מלאה השתמשו ב-`Header` + `to_bytes_with_header`.

## פרופילי Feature עיקריים

- `packed-seq` – מיקבול רצפים בבלוק רציף.
- `packed-struct` – מיקבול שדות struct.
- `compact-len` – Varint Length (7-bit).
- `compression` / `gpu-compression`.
- `schema-structural` – hash סכמה באמצעות Norito JSON מקנון.

## פריסות לפי סוג

- **פרימיטיבים:** LE Fix.
- **Strings/Bytes:** `[len][data]` עם Varint/Fix.
- **Option/Result:** תג `u8` + Payload.
- **Arrays/Tuples:** בהתאם לגודל קבוע/משתנה.
- **`Vec<T>`:** Pack (Varint/Offsets) לפי דגלים.
- **Maps/Sets:** נקראים כזוגות.
- **Struct:** לפי derive-packing או Layout רגיל.
- **Enums:** Tag + Variant Payload.
- **Newtype:** מייצאים בבירור.

## התאמה JSON

- `norito::json::{from_str,to_string,json!}` מותאמי Norito.
- המספקים: הקפדה על float/flags מותאמים ל-`bin`/`json`.

## מודול CRC64

- פולינום ECMA `0x42F0E1EBA9EA3693`.
- Detects SSE4.2/PMULL.
- APIs: `crc64`,‏ `hardware_crc64`,‏ `crc64_fallback`.

## דחיסה

- `to_compressed_bytes`,‏ `from_compressed_bytes`.
- `to_bytes_auto`: בחירה מסתגלת בין ללא דחיסה, CPU zstd, GPU zstd.
- סף דיפולטי: <2KiB – ללא דחיסה; ≥1MiB – GPU אפשרי; CPU ברמות זסטד שונות.

## פנקס Decode/Flags

- דגלים: `COMPACT_LEN`,‏ `COMPACT_SEQ_LEN`,‏ `VARINT_OFFSETS`,‏ `PACKED_STRUCT`,‏ `PACKED_SEQ`,‏ `HEADERLESS`.
- `DecodeFlagsGuard` + `PayloadCtxGuard`.
- `reset_decode_state` לאיפוס.

## שגיאות Norito

- `Error`: ‎`Io`, `InvalidMagic`, `UnsupportedVersion`, `UnsupportedFlag`, `ChecksumMismatch`, `SchemaMismatch`, `UnexpectedEof`, `InvalidValue`, `Utf8Error`, `DecodePanic`.
- מחלקה `BarePayload` לשגיאות ללא כותרת.
- כל Decode מחזיר תוצאה עם שגיאה מפורטת (לא `panic`).

## אכיפת סכמה

- `schema_hash_structural`.
- Norito JSON קנוני כדי לשמור התאמה בין Rust/Python/Java/Swift.
- הקפדה על hash יציב; שינוי דרוש עדכון fixtures.

## כיסוי בדיקות

- `crates/norito/tests`:
  - Roundtrip לבדם, Packed Seq, CRC64.
  - Header flags, CRC failure, compression.
  - Schema hash parity (Python).
- `integration_tests/norito_*` לפוליסות.
- `docs/assets/norito_examples/*` משמש כ-fixtures.

## כלי עזר

```bash
cargo test -p norito             # יחידות
cargo test -p norito --features packed-seq,compact-len
cargo test -p norito --features compression
python3 scripts/norito_json_diff.py docs/source/norito_json_inventory.json
scripts/check_norito_features.sh # כיסוי דגלים
```

## מגבלות ו-Roadmap

- Highlight: Packed Seq + GPU compression טרם בייצור.
- Strict-safe guard מאט ביצועים ~20%.
- Columnar heuristics – זקוק לבנצ׳ים נוספים (see `norito_crc64_parity_bench.md`).
- JSON adapters נמצאים בהרחבה אך נשארים `Value`.
- חתימות Rust derive עודכן; Swift/Python bindings מחכים ל-ffi refresh.

## מקורות נוספים

- `crates/norito/src/core/header.rs`
- `crates/norito/src/codec/mod.rs`
- `crates/norito/src/json/mod.rs`
- `crates/norito/src/compression/mod.rs`
- `docs/source/norito_crc64_parity_bench.md`
- `docs/source/norito_json_inventory.md`
- `docs/source/norito_streaming.md`
- `docs/source/norito_streaming_transport_design.md`

</div>
