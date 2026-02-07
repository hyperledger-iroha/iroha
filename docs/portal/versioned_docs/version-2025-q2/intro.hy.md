---
lang: hy
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/intro.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e797879d1f77c8cfd62fcc67874d584f6bdeee9395faafe52fc33f26ce2e6a21
source_last_modified: "2025-12-29T18:16:35.904811+00:00"
translation_last_reviewed: 2026-02-07
---

# Welcome to the SORA Nexus Developer Portal

The SORA Nexus developer portal bundles interactive documentation, SDK
tutorials, and API references for Nexus operators and Hyperledger Iroha
contributors. It complements the main docs site by surfacing hands-on guides
and generated specs directly from this repository.

## What you can do here

- **Learn Norito** – start with the overview and quickstart to understand the
  serialization model and bytecode tooling.
- **Bootstrap SDKs** – follow quickstarts for JavaScript and Rust today; Python,
  Swift, and Android guides will join them as recipes are migrated.
- **Browse API references** – the Torii OpenAPI page renders the latest REST
  specification, and configuration tables link back to the canonical Markdown
  sources.
- **Prepare deployments** – operational runbooks (telemetry, settlement, Nexus
  overlays) are being ported from `docs/source/` and will land in this site as
  the migration progresses.

## Current status

- ✅ Docusaurus v3 scaffolding with live pages for Norito and SDK quickstarts.
- ✅ Torii OpenAPI plugin wired to `npm run sync-openapi`.
- ⏳ Migrating the remaining guides from `docs/source/`.
- ⏳ Adding preview builds and linting to the documentation CI.

## Getting involved

- See `docs/portal/README.md` for local development commands (`npm install`,
  `npm run start`, `npm run build`).
- Content migration tasks are tracked alongside the `DOCS-*` roadmap items.
  Contributions are welcome—port sections from `docs/source/` and add the page
  to the sidebar.
- If you add a generated artifact (specs, config tables), document the build
  command so future contributors can refresh it easily.
