---
lang: az
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI imzalanması
---------------

- Torii OpenAPI spesifikasiyası (`torii.json`) imzalanmalı və manifest `cargo xtask openapi-verify` tərəfindən təsdiqlənməlidir.
- İcazə verilən imza açarları `allowed_signers.json`-də yaşayır; imza açarı dəyişdikdə bu faylı döndərin. `version` sahəsini `1`-də saxlayın.
- CI (`ci/check_openapi_spec.sh`) artıq həm ən son, həm də cari xüsusiyyətlər üçün icazə siyahısını tətbiq edir. Başqa portal və ya boru kəməri imzalanmış spesifikasiyanı istehlak edərsə, sürüşmənin qarşısını almaq üçün onun doğrulama addımını eyni icazə siyahısı faylına yönəldin.
- Açar çevrildikdən sonra yenidən imzalamaq üçün:
  1. `allowed_signers.json`-i yeni açıq açarla yeniləyin.
  2. Spesifikasiyanı bərpa edin/imzalayın: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Manifestin icazə verilən siyahıya uyğun olduğunu təsdiqləmək üçün `ci/check_openapi_spec.sh` (və ya `cargo xtask openapi-verify` əl ilə) yenidən işə salın.