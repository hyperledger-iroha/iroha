---
lang: ur
direction: rtl
source: docs/source/sns/suffix_catalog.md
status: needs-translation
generator: scripts/sync_docs_i18n.py
---

# Static SNS suffix catalog retired

> Safety notice (2026-07-19): the former localization contained a fixed
> `.sora` / `.nexus` / `.dao` table and numeric 1/2/3 mappings that are not
> part of the first-release alias model. Do not use an archived copy as runtime
> configuration or provisioning input.

External alias names are catalog-free. Text-to-`DataSpaceId` mappings are
resolved from configured static mappings and active SNS records, and live SNS
policies are available only through the read-only policy surface. Setup and
lifecycle changes use the signed `iroha app alias` planner workflow; there are
no SNS mutation routes.

Use the current canonical English guide:
[`suffix_catalog.md`](./suffix_catalog.md).
