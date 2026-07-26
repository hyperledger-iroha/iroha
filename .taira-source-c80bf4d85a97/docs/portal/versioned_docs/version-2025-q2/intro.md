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
