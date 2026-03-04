---
lang: he
direction: rtl
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb88618a92dc54bfc2091c10ab45a4b90d69c4b767e5d48c4a3ba6812a13467f
source_last_modified: "2026-01-30T09:44:29.106026+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/crypto/attachments/sm_openssl_provenance.md -->

% SM OpenSSL/Tongsuo Provenance Snapshot
% Generated: 2026-01-30

# סיכום סביבה

- `pkg-config --modversion openssl`: `3.6.0`
- `openssl version -a`: מדווח `LibreSSL 3.3.6` (ערכת TLS מערכתית ב‑macOS).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"`: ראו
  `sm_iroha_crypto_tree.txt` עבור ערימת התלויות המדויקת (crate `openssl` v0.10.74,
  `openssl-sys` v0.9.x, מקורות OpenSSL 3.x vendored זמינים דרך crate `openssl-src`;
  הפיצ'ר `vendored` מופעל ב‑`crates/iroha_crypto/Cargo.toml` עבור בניות preview דטרמיניסטיות).

# הערות

- סביבת פיתוח מקומית מקושרת לכותרות/ספריות LibreSSL; בניות preview בייצור חייבות
  להשתמש ב‑OpenSSL >= 3.0.0 או Tongsuo 8.x. החליפו את ערכת המערכת או הגדירו
  `OPENSSL_DIR`/`PKG_CONFIG_PATH` בעת יצירת חבילת הארטיפקט הסופית.
- צרו מחדש snapshot זה בתוך סביבת הבנייה לשחרור כדי ללכוד את hash ה‑tarball המדויק
  של OpenSSL/Tongsuo (`openssl version -v`, `openssl version -b`, `openssl version -f`)
  ולצרף את סקריפט/ה‑checksum של בנייה רב‑שחזורית. בבניות vendored, תעדו את
  גרסת/commit ה‑crate `openssl-src` שבה Cargo משתמש (גלוי ב‑
  `target/debug/build/openssl-sys-*/output`).
- מארחי Apple Silicon דורשים `RUSTFLAGS=-Aunsafe-code` בעת הרצת ה‑OpenSSL smoke
  harness כדי ש‑stubs של האצת SM3/SM4 ל‑AArch64 יתקמפו (ה‑intrinsics אינם זמינים
  ב‑macOS). הסקריפט `scripts/sm_openssl_smoke.sh` מייצא דגל זה לפני הפעלת `cargo`
  כדי לשמור על עקביות בין CI והרצות מקומיות.
- צרפו מקוריות upstream (למשל, SHA256 של `openssl-src-<ver>.tar.gz`) לאחר
  קיבוע צינור האריזה; השתמשו באותו hash בארטיפקטים של CI.

</div>
