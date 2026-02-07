---
lang: kk
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI қол қою
---------------

- Torii OpenAPI спецификациясына (`torii.json`) қол қою керек және манифест `cargo xtask openapi-verify` арқылы расталады.
- рұқсат етілген қол қоюшы кілттері `allowed_signers.json` ішінде өмір сүреді; қол қою кілті өзгерген сайын бұл файлды бұрыңыз. `version` өрісін `1` күйінде сақтаңыз.
- CI (`ci/check_openapi_spec.sh`) ең соңғы және ағымдағы сипаттамалар үшін рұқсат етілген тізімді әлдеқашан бекітеді. Егер басқа портал немесе конвейер қол қойылған спецификацияны пайдаланса, жылжуды болдырмау үшін оның тексеру қадамын бірдей рұқсат етілген тізім файлына көрсетіңіз.
- Кілтті айналдырғаннан кейін қайта қол қою үшін:
  1. `allowed_signers.json` жаңа ашық кілтпен жаңартыңыз.
  2. Спецификацияны қайта жасаңыз/қол қойыңыз: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  3. Манифест рұқсат етілген тізімге сәйкес келетінін растау үшін `ci/check_openapi_spec.sh` (немесе `cargo xtask openapi-verify` қолмен) қайта іске қосыңыз.