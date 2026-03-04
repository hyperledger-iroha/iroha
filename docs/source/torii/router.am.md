---
lang: am
direction: ltr
source: docs/source/torii/router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 055a61ceb91aca67ec439e50f64fb6eb2ba70723ec67109ccaf79ebfe306e90b
source_last_modified: "2026-01-05T09:28:12.105600+00:00"
translation_last_reviewed: 2026-02-07
---

# Torii Router Composition

Torii exposes its HTTP surface through a staged builder that groups related
routes and wires shared state exactly once. The `RouterBuilder` helper lives in
`iroha_torii::router::builder` and wraps `axum::Router<SharedAppState>`. Each
`Torii::add_*_routes` helper now accepts `&mut RouterBuilder` and performs its
registrations via `RouterBuilder::apply`.

## Builder Pattern

The builder API removes the old "return a new `Router`" pattern. Instead of
reassigning a router after every helper, callers invoke the helpers in sequence:

```rust
let mut builder = RouterBuilder::new(app_state.clone());

// Feature-neutral groups
torii.add_sumeragi_routes(&mut builder);
torii.add_telemetry_routes(&mut builder);
torii.add_core_info_routes(&mut builder);

// Optional feature groups
#[cfg(feature = "connect")]
torii.add_connect_routes(&mut builder);
#[cfg(feature = "app_api")]
torii.add_app_api_routes(&mut builder);

let router = builder.finish();
```

This approach guarantees that:

- The shared `AppState` is cloned only when needed (`apply_with_state` remains
  available for handlers that require it).
- Feature-gated helpers compose deterministically regardless of compile-time
  configuration.
- Tests can exercise different combinations without juggling intermediate
  `Router` values.

## Regression Tests

The integration test `crates/iroha_torii/tests/router_feature_matrix.rs`
constructs Torii with a minimal in-memory stack and ensures the router builds
under the active feature set. When the schema/OpenAPI endpoint is compiled in,
the test optionally diffs a generated specification against a snapshot:

- Set `IROHA_TORII_OPENAPI_EXPECTED=/path/to/openapi.json` to assert the
  generated document matches the snapshot.
- Optionally set `IROHA_TORII_OPENAPI_ACTUAL=/tmp/iroha-openapi.json` to write
  the current output for manual inspection or diff tooling.

If the OpenAPI endpoint is unavailable the diff is skipped automatically. The
test also performs feature-gated smoke requests (e.g., `/v1/domains` under
`app_api`, `/v1/connect/status` under `connect`) to verify the staged builder
covers every route group.

## Migration Notes

Projects embedding Torii should migrate any direct `Router` manipulations to
`RouterBuilder` helpers. The historical pattern of returning a modified router from

## Further Reading

- `docs/source/torii/app_api_parity_audit.md` — parity matrix covering the `app_api` and `connect` route groups, their DTOs, and existing test coverage.
