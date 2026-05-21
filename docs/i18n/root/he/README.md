---
lang: he
direction: rtl
source: README.md
status: complete
translator: manual
source_hash: 8f2fe1d4fc449fc895f770195f3d209d5a576dfe78c8fea37c523cc111694c44
source_last_modified: "2026-02-07T00:00:00+00:00"
translation_last_reviewed: 2026-02-07
---

<div dir="rtl">

# Hyperledger Iroha

[![רישיון](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha היא פלטפורמת בלוקצ'יין דטרמיניסטית עבור פריסות מורשות וקונסורציומיות. היא מספקת ניהול חשבונות ונכסים, הרשאות on-chain וחוזים חכמים באמצעות Iroha Virtual Machine (IVM).

> מצב ה-workspace והשינויים האחרונים מתועדים ב-[`status.md`](./status.md).

## קווי שחרור

מאגר זה מספק שני מסלולי פריסה מאותה בסיס קוד:

- **Iroha 2**: רשתות מורשות/קונסורציום בפריסה עצמית.
- **Iroha 3 (SORA Nexus)**: מסלול מוכוון Nexus המשתמש באותם crates מרכזיים.

שני המסלולים חולקים את אותם רכיבי ליבה, כולל סריאליזציית Norito, קונצנזוס Sumeragi ושרשרת הכלים Kotodama -> IVM.

## מבנה המאגר

- [`crates/`](./crates): crates עיקריים ב-Rust (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `ivm`, `norito` ועוד).
- [`integration_tests/`](./integration_tests): בדיקות אינטגרציה ורשת חוצות-רכיבים.
- [`IrohaSwift/`](./IrohaSwift): חבילת SDK ל-Swift.
- [`java/iroha_android/`](./java/iroha_android): חבילת SDK לאנדרואיד.
- [`docs/`](./docs): תיעוד למשתמשים, תפעול ומפתחים.

## התחלה מהירה

### דרישות מקדימות

- [Rust stable](https://www.rust-lang.org/tools/install)
- אופציונלי: Docker + Docker Compose להרצות מקומיות מרובות peer

### בנייה ובדיקות (workspace)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

הערות:

- בניית כל ה-workspace עשויה להימשך כ-20 דקות.
- בדיקות מלאות של ה-workspace עשויות להימשך מספר שעות.
- ה-workspace מכוון ל-`std` (בניות WASM/no-std אינן נתמכות).

### פקודות בדיקה ממוקדות

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### פקודות בדיקה ל-SDK

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## הפעלת רשת מקומית

הפעילו את רשת Docker Compose שסופקה:

```bash
docker compose -f defaults/docker-compose.yml up
```

השתמשו ב-CLI עם תצורת הלקוח ברירת המחדל:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

לשלבי פריסה מקומית של הדמון, ראו [`crates/irohad/README.md`](./crates/irohad/README.md).

## API ונראות תפעולית

Torii חושף גם API של Norito וגם JSON API. נקודות קצה נפוצות לתפעול:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

לתיעוד מלא של נקודות הקצה:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Crates מרכזיים

- [`crates/iroha`](./crates/iroha): ספריית לקוח.
- [`crates/irohad`](./crates/irohad): בינארים של דמון peer.
- [`crates/iroha_cli`](./crates/iroha_cli): CLI ייחוס.
- [`crates/iroha_core`](./crates/iroha_core): מנוע הביצוע וליבת ה-ledger.
- [`crates/iroha_config`](./crates/iroha_config): מודל תצורה טיפוסי.
- [`crates/iroha_data_model`](./crates/iroha_data_model): מודל נתונים קנוני.
- [`crates/iroha_crypto`](./crates/iroha_crypto): פרימיטיבים קריפטוגרפיים.
- [`crates/norito`](./crates/norito): קודק סריאליזציה דטרמיניסטי.
- [`crates/ivm`](./crates/ivm): Iroha Virtual Machine.
- [`crates/iroha_kagami`](./crates/iroha_kagami): כלי מפתחות/genesis/תצורה.

## מפת תיעוד

- אינדקס ראשי: [`docs/README.md`](./docs/README.md)
- Genesis: [`docs/genesis.md`](./docs/genesis.md)
- קונצנזוס (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- צינור עיבוד עסקאות: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- פנימיות P2P: [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM syscalls: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- דקדוק Kotodama: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- פורמט wire של Norito: [`norito.md`](./norito.md)
- מעקב עבודה נוכחי: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## תרגומים

סקירה ביפנית: [`README.ja.md`](./README.ja.md)

סקירות נוספות:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

תהליך תרגום: [`docs/i18n/README.md`](./docs/i18n/README.md)

## תרומה ועזרה

- מדריך תרומה: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- ערוצי קהילה/תמיכה: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## רישיון

Iroha מופץ תחת Apache-2.0. ראו [`LICENSE`](./LICENSE).

התיעוד מופץ תחת CC-BY-4.0: http://creativecommons.org/licenses/by/4.0/

</div>
