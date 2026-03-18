---
lang: mn
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI гарын үсэг зурж байна
---------------

- Torii OpenAPI техникийн (`torii.json`) гарын үсэг зурсан байх ёстой бөгөөд манифестийг `cargo xtask openapi-verify` баталгаажуулсан байна.
- Зөвшөөрөгдсөн гарын үсэг зурсан түлхүүрүүд `allowed_signers.json` дээр амьдардаг; гарын үсэг зурах түлхүүр өөрчлөгдөх бүрт энэ файлыг эргүүлнэ. `version` талбарыг `1` дээр байлга.
- CI (`ci/check_openapi_spec.sh`) нь хамгийн сүүлийн үеийн болон одоогийн үзүүлэлтүүдийн аль алинд нь зөвшөөрөгдсөн жагсаалтыг аль хэдийн хэрэгжүүлсэн. Хэрэв өөр портал эсвэл дамжуулах хоолой нь гарын үсэг зурсан үзүүлэлтийг хэрэглэж байгаа бол зөрөхгүйн тулд баталгаажуулах алхамыг зөвшөөрөгдсөн жагсаалтын ижил файл руу чиглүүлээрэй.
- Түлхүүрийг эргүүлсний дараа дахин гарын үсэг зурахын тулд:
  1. `allowed_signers.json`-г шинэ нийтийн түлхүүрээр шинэчил.
  2. Тодорхойлолтыг дахин үүсгэх/гарын үсэг зурах: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Манифест зөвшөөрөгдсөн жагсаалтад таарч байгааг баталгаажуулахын тулд `ci/check_openapi_spec.sh` (эсвэл `cargo xtask openapi-verify` гараар) дахин ажиллуулна уу.