---
lang: uz
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI imzolash
---------------

- Torii OpenAPI (`torii.json`) imzolanishi kerak va manifest `cargo xtask openapi-verify` tomonidan tasdiqlangan.
- Ruxsat berilgan imzo kalitlari `allowed_signers.json` da ishlaydi; imzolash kaliti har o'zgarganda ushbu faylni aylantiring. `version` maydonini `1` da saqlang.
- CI (`ci/check_openapi_spec.sh`) allaqachon so'nggi va joriy xususiyatlar uchun ruxsat etilgan ro'yxatni joriy qiladi. Agar boshqa portal yoki quvur liniyasi imzolangan spetsifikatsiyani iste'mol qilsa, siljishning oldini olish uchun uning tekshirish bosqichini xuddi shu ruxsat etilgan ro'yxat fayliga yo'naltiring.
- Kalitni aylantirgandan keyin qayta imzolash uchun:
  1. `allowed_signers.json` ni yangi ochiq kalit bilan yangilang.
  2. Spetsifikatsiyani qayta yarating/imzolang: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Manifest ruxsat etilgan roʻyxatga mos kelishini tasdiqlash uchun `ci/check_openapi_spec.sh` (yoki qoʻlda `cargo xtask openapi-verify`) ni qayta ishga tushiring.