---
lang: he
direction: rtl
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-11-16T17:11:03.605418+00:00"
translation_last_reviewed: 2026-01-30
---

חתימת OpenAPI
--------------

- מפרט OpenAPI של Torii (`torii.json`) חייב להיות חתום, וה‑manifest מאומת באמצעות `cargo xtask openapi-verify`.
- מפתחות החתימה המותרים נמצאים ב‑`allowed_signers.json`; יש לסובב את הקובץ הזה בכל שינוי של מפתח החתימה. השאירו את השדה `version` בערך `1`.
- ה‑CI (`ci/check_openapi_spec.sh`) כבר אוכף את allowlist עבור המפרטים latest ו‑current. אם פורטל או צינור נוסף צורך את המפרט החתום, כוונו את שלב האימות לאותו קובץ allowlist כדי למנוע סטייה.
- כדי לחתום מחדש לאחר סבב מפתחות:
  1. עדכנו את `allowed_signers.json` עם המפתח הציבורי החדש.
  2. צרו מחדש וחתמו את המפרט: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. הריצו שוב את `ci/check_openapi_spec.sh` (או `cargo xtask openapi-verify` ידנית) כדי לוודא שה‑manifest תואם ל‑allowlist.
