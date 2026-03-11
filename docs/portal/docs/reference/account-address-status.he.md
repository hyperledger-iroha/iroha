---
lang: he
direction: rtl
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f18b4a15e42363483c4f65945aba2cc208f9a2e59b6f31f143e5dc792d8d9071
source_last_modified: "2025-11-27T10:35:59.095888+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: account-address-status
title: ציות כתובת חשבון
description: סיכום זרימת העבודה של fixture ADDR-2 וכיצד צוותי SDK נשארים מסונכרנים.
---

חבילת ה-ADDR-2 הקנונית (`fixtures/account/address_vectors.json`) כוללת fixtures של I105 and i105-default (`sora`; half/full width), multisignature ו-negative. כל משטח SDK + Torii מסתמך על אותו JSON כדי לזהות כל סטייה של codec לפני הגעה לפרודקשן. דף זה משקף את תקציר הסטטוס הפנימי (`docs/source/account_address_status.md` בשורש המאגר) כדי שקוראי הפורטל יוכלו לעיין בזרימה בלי לחפור ב-mono-repo.

## יצירה מחדש או אימות של החבילה

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` — פולט את ה-JSON ל-stdout לבדיקה אד-הוק.
- `--out <path>` — כותב לנתיב אחר (למשל בעת השוואת שינויים מקומית).
- `--verify` — משווה את העותק העובד לתוכן שנוצר מחדש (לא ניתן לשלב עם `--stdout`).

זרימת ה-CI **Address Vector Drift** מריצה `cargo xtask address-vectors --verify`
בכל שינוי של ה-fixture, הגנרטור או ה-docs כדי להתריע למבקרים מיד.

## מי צורך את ה-fixture?

| Surface | Validation |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

כל harness מבצע round-trip של הבייטים הקנוניים + I105 + קידודים דחוסים ובודק שקודי השגיאה בסגנון Norito תואמים ל-fixture עבור המקרים השליליים.

## צריכים אוטומציה?

כלי ה-release יכולים לסקריפט רענוני fixture עם ה-helper
`scripts/account_fixture_helper.py`, שמביא או מאמת את החבילה הקנונית בלי שלבי copy/paste:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

ה-helper מקבל overrides של `--source` או משתנה הסביבה `IROHA_ACCOUNT_FIXTURE_URL` כדי ש-CI של SDK יוכל להצביע למראה המועדפת. כאשר מספקים `--metrics-out`, ה-helper כותב `account_address_fixture_check_status{target="…"}` יחד עם digest SHA-256 קנוני (`account_address_fixture_remote_info`) כדי ש-Prometheus textfile collectors ולוח Grafana `account_address_fixture_status` יוכלו להוכיח שכל משטח נשאר מסונכרן. יש להתריע כאשר יעד מדווח `0`. לאוטומציה מרובת משטחים השתמשו בעטיפה `ci/account_fixture_metrics.sh` (מקבל `--target label=path[::source]` חוזרים) כדי שצוותי הכוננות יפרסמו קובץ `.prom` מאוחד עבור textfile collector של node-exporter.

## צריכים את התקציר המלא?

סטטוס התאימות המלא של ADDR-2 (owners, תוכנית ניטור, פריטי פעולה פתוחים)
נמצא ב-`docs/source/account_address_status.md` בתוך המאגר יחד עם Address Structure RFC (`docs/account_structure.md`). השתמשו בדף הזה כתזכורת תפעולית מהירה; להנחיות מעמיקות, פנו למסמכי המאגר.
