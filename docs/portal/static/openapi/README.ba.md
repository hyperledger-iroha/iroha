---
lang: ba
direction: ltr
source: docs/portal/static/openapi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8ad316fefd99c4c3b9ddbade7de59f12aa2dbe9ee256784f61ac87bb4341f04a
source_last_modified: "2025-12-29T18:16:35.902041+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

OpenAPI ҡул ҡуйыу
---------------.

- Torii OpenAPI спец (`torii.json`) ҡул ҡуйырға тейеш, ә манифест `cargo xtask openapi-verify` тарафынан раҫлана.
- Ҡултамға асҡыстары рөхсәт ителгән `allowed_signers.json`-та йәшәй; был файлды әйләндерергә, ҡасан ҡул ҡуйыу төймәһе үҙгәрештәр. `version` яланын `1`-та һаҡлағыҙ.
- CI (`ci/check_openapi_spec.sh`) инде һуңғы һәм ағымдағы спецификациялар өсөн рөхсәт ҡағыҙын үтәй. Әгәр ҙә башҡа портал йәки торба үткәргес ҡул ҡуйылған спец, уның тикшерелгән аҙымын күрһәтегеҙ, шул уҡ рөхсәт исемлеге файлында дрейфтан ҡотолоу өсөн.
- Асҡыс әйләнешенән һуң яңынан ҡул ҡуйыу:
  1. Яңы асыҡ асҡыс менән `allowed_signers.json` яңыртыу.
  2. Регенерация/билдәләү спец: `NORITO_SKIP_BINDINGS_SYNC=1 cargo xtask openapi --output docs/portal/static/openapi/torii.json --sign <ed25519-key-hex-path>`.
  .