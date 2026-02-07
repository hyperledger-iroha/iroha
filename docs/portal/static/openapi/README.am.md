---
lang: am
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI መፈረም
------------

- Torii OpenAPI spec (`torii.json`) መፈረም አለበት፣ እና አንጸባራቂው በ`cargo xtask openapi-verify` የተረጋገጠ ነው።
- የተፈቀዱ የፈራሚ ቁልፎች በ `allowed_signers.json` ውስጥ ይኖራሉ; የመፈረሚያ ቁልፉ በተለወጠ ቁጥር ይህንን ፋይል ያሽከርክሩት። የ `version` መስኩን በ `1` ያቆዩት።
- CI (`ci/check_openapi_spec.sh`) ለሁለቱም የቅርብ ጊዜ እና ወቅታዊ ዝርዝሮች የፍቃድ ዝርዝሩን አስቀድሞ ያስፈጽማል። ሌላ ፖርታል ወይም የቧንቧ መስመር የተፈረመውን ዝርዝር ከበላ፣ መንሸራተትን ለማስቀረት የማረጋገጫ ደረጃውን በተመሳሳዩ የፈቃድ ዝርዝር ፋይል ላይ ያመልክቱ።
- ከቁልፍ ማሽከርከር በኋላ እንደገና ለመፈረም;
  1. `allowed_signers.json`ን በአዲሱ የህዝብ ቁልፍ ያዘምኑ።
  2. መግለጫውን ያድሱ/ይፈርሙ፡ `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`።
  3. አንጸባራቂው ከተፈቀዱ ዝርዝሩ ጋር የሚዛመድ መሆኑን ለማረጋገጥ `ci/check_openapi_spec.sh` (ወይም `cargo xtask openapi-verify` በእጅ) እንደገና ያሂዱ።