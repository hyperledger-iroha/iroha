# First-release query completion goals

This work completes the Torii, SDK and CLI query redesign and its six follow-ups.
The contract is [collection_queries.md](collection_queries.md). Retired request
shapes are removed, without aliases, compatibility decoders or ignored controls.

| Goal | Owner | Completion criteria |
| --- | --- | --- |
| Q1: Reach all committed history | Core/Kura/Torii | Authenticated checkpoints bound off-chain reads independently of distance from the tip; restart rebuilds them; stale entries cannot authorize foreign history; consensus metering never depends on the cache. Focused cold/warm, budget, corruption and restart tests pass. |
| Q2: Exact routed collections | Torii | Execute each collection once over the caller-visible state; counts and aggregates neither duplicate rows nor omit visible routes. Projection and memory bounds remain enforced. Paging preserves scope and authorization. |
| Q3: Bounded indexed reads | Torii/Core | Identity-ordered reads seek by typed key, exact identifiers use point lookups, and accounts stream. Skipped entries and lookahead consume budgets; cursors disclose only visible keys; totals describe the complete query on every page. Regression tests cover both directions and sparse matches. |
| Q4: One collection contract | Torii/SDKs/CLI/MCP | Permissions, subscription plans, subscriptions, UAID manifests, explorer feeds, account history and contract activity use the shared controls and page envelope. Retire redundant transaction-history routes. Update OpenAPI, MCP, SDKs, CLI and consumers together; reject retired controls explicitly. |
| Q5: Honest signed queries | Model/Core/Rust SDK/CLI | One selector layout independent of Cargo features, canonical `FindAssetDefinitions` spelling, every admitted source expressible, precise unsupported-shape errors and no ignored routing parameters. Wire/identity fixtures, docs and consumers agree. Client network context is explicit and validated centrally. |
| Q6: SDK grammar parity | Shared/SDKs | Rust, Swift, Kotlin/Java, JavaScript, Python and C# agree on text and JSON filters, exact numbers, quoted field paths and control-character handling against shared vectors. Iterators continue through short and empty pages. |
| Q7: Verify the integrated result | All owners | Focused suites, affected compilation, formatting, codec/feature guards and generated-contract checks pass on the combined changes. Record unrelated failures separately; whole-node or release readiness requires its own current-candidate evidence. |

Implementation and tests are authoritative. Keep current health and remaining
outcomes in the root status and roadmap; routine command transcripts belong in
the change report rather than this specification.
