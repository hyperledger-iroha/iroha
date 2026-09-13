# Musubi status before the contract workflow candidate

Exact prior status paragraph retained on 2026-09-13. These counts qualify the earlier local candidate, not the current deployment workflow.

Musubi now resolves local workspace graphs without a registry configuration or cache, with an explicit local graph commitment distinct from authenticated registry locks. Native Unix descriptor reads support macOS without Linux descriptor-path assumptions; standalone contract tests use immutable, manifest-declared source sets through the shared typed compiler. Offline registry selection binds the explicit deployment and account profile. The rebuilt coffee example passes four VM tests and produces IVM bytecode with Musubi alone. Full Musubi and Kotodama library runs pass 423 and 1,093 tests respectively (one existing Musubi ignore), and both new IVM source-set regressions pass. The public tutorial and 20 translations pass their scoped checks. This is local evidence, not full-workspace or live-publication qualification; see the [Musubi contract](specs/musubi.md) and [runnable example](examples/coffee-club/README.md).

## Previous roadmap row

| P5 | Musubi registry and publication deployment | Musubi service, Core/Torii, cache and deploy owners | Qualify the explicit local/registry lock model and immutable standalone test graph across supported release hosts; complete long-detach/private HTTPS runner and authenticated recovery integration; descriptor-relative/no-follow atomic cache/publication and supported non-Unix guarantees; authoritative metrics, queue crash/restart, four-peer and namespace/two-week soak; supported-host whole-process 64 MiB target. See [Musubi contract](specs/musubi.md). |
