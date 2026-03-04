---
lang: hy
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI ստորագրում
----------------

- Torii OpenAPI բնութագրիչը (`torii.json`) պետք է ստորագրված լինի, իսկ մանիֆեստը հաստատված է `cargo xtask openapi-verify`-ով:
- Թույլատրված ստորագրող ստեղները գործում են `allowed_signers.json`-ում; պտտել այս ֆայլը, երբ ստորագրման բանալին փոխվի: Պահպանեք `version` դաշտը `1`-ում:
- CI (`ci/check_openapi_spec.sh`) արդեն պարտադրում է թույլտվությունների ցանկը թե՛ վերջին, թե՛ ընթացիկ ակնարկների համար: Եթե ​​մեկ այլ պորտալ կամ խողովակաշար սպառում է ստորագրված հատկանիշը, ուղղեք դրա ստուգման քայլը նույն թույլտվությունների ցանկի ֆայլի վրա՝ շեղումից խուսափելու համար:
- Բանալին պտտելուց հետո նորից ստորագրելու համար.
  1. Թարմացրեք `allowed_signers.json`-ը նոր հանրային բանալիով:
  2. Վերականգնել/ստորագրել հատկանիշը՝ `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`:
  3. Կրկին գործարկեք `ci/check_openapi_spec.sh` (կամ `cargo xtask openapi-verify` ձեռքով)՝ հաստատելու, որ մանիֆեստը համապատասխանում է թույլտվությունների ցանկին: