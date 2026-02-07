---
lang: my
direction: ltr
source: docs/portal/docs/intro.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Welcome to the SORA Nexus Developer Portal

The SORA Nexus developer portal bundles interactive documentation, SDK
tutorials, and API references for Nexus operators and Hyperledger Iroha
contributors. It complements the main docs site by surfacing hands-on guides
and generated specs directly from this repository. The landing page now carries
themed Norito/SoraFS entry points, signed OpenAPI snapshots, and a dedicated
Norito Streaming reference so contributors can find the streaming control-plane
contract without digging through the root spec.

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

- ✅ Themed Docusaurus v3 landing with refreshed typography, gradient-driven
  hero/cards, and resource tiles that include the Norito Streaming summary.
- ✅ Torii OpenAPI plugin wired to `npm run sync-openapi`, with signed snapshot
  checks and CSP guards enforced by `buildSecurityHeaders`.
- ✅ Preview and probe coverage run in CI (`docs-portal-preview.yml` +
  `scripts/portal-probe.mjs`), now gating the streaming doc, SoraFS quickstarts,
  and the reference checklists before artifacts are published.
- ✅ Norito, SoraFS, and SDK quickstarts plus reference sections are live in the
  sidebar; new imports from `docs/source/` (streaming, orchestration, runbooks)
  land here as they are authored.

## Getting involved

- See `docs/portal/README.md` for local development commands (`npm install`,
  `npm run start`, `npm run build`).
- Content migration tasks are tracked alongside the `DOCS-*` roadmap items.
  Contributions are welcome—port sections from `docs/source/` and add the page
  to the sidebar.
- If you add a generated artifact (specs, config tables), document the build
  command so future contributors can refresh it easily.
