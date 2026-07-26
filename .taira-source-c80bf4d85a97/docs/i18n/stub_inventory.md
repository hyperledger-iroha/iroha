# Translation Stub Inventory

Snapshot date (UTC): 2026-04-02

Criteria: files that still contain `status: needs-translation` (or
`#+STATUS: needs-translation` for Org mode) and are managed by the repo
localization workflow.

Current state:

- Total unfinished translation files: `0`
- Configured target locales with unfinished files: `0`
- Unfinished files per locale: none; every managed translation target now has
  non-stub content.
- Published locales currently remain `["en"]` via
  `docs/i18n/published_locales.json`, so stub-only locales are not shipping.

## Totals by locale

| Locale | Stub files |
| --- | ---: |
| Total | 0 |

## Directory breakdown

| Directory | Stub files | Notes |
| --- | ---: | --- |
| repo-root | 0 | No managed repo-root translation stubs remain. |
| docs/formal | 0 | No managed `docs/formal` translation stubs remain. |
| docs/portal/docs | 0 | No managed portal sibling translation stubs remain. |
| docs/portal/i18n | 0 | No managed portal locale mirror stubs remain. |
| docs/source | 0 | No managed `docs/source` translation stubs remain. |
| Total | 0 |  |

## Outstanding source documents

No managed source documents remain in `needs-translation` state.

## Notes

- This inventory reflects translation completeness, not source-document
  correctness. Review the English source files themselves before translating if
  content ownership is unclear.
- `2026-annual-hyperledger-iroha.md` remains intentionally excluded from the
  managed translation manifest per user instruction.
- A separate refresh backlog still exists for already-`complete` translations
  whose `source_hash` no longer matches the current English source. That is not
  a stub-count issue and is tracked separately from this inventory.
- The current sync tooling (`python3 scripts/sync_docs_i18n.py` and
  `node docs/portal/scripts/sync-i18n.mjs`) guarantees parity of stub presence
  and metadata, but it does not produce reviewed translations.
