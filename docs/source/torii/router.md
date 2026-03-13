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
test also performs feature-gated smoke requests (e.g., `/v2/domains` under
`app_api`, `/v2/connect/status` under `connect`) to verify the staged builder
covers every route group.

## Migration Notes

Projects embedding Torii should migrate any direct `Router` manipulations to
`RouterBuilder` helpers.
The historical pattern of returning a modified router from each `add_*_routes`
helper has been replaced by in-place builder composition.

Recommended migration pattern:

1. Construct `RouterBuilder::new(app_state.clone())`.
2. Call the `Torii::add_*_routes` helpers in the desired order.
3. Register custom routes with `builder.apply(...)` (or
   `builder.apply_with_state(...)` when extra state is required).
4. Finalize with `builder.finish()`.

This keeps route registration deterministic across feature flags and avoids
reassigning intermediate `Router` values.

## Further Reading

- `docs/source/torii/app_api_parity_audit.md` — parity matrix covering the `app_api` and `connect` route groups, their DTOs, and existing test coverage.
- `crates/iroha_torii/docs/mcp_api.md` — canonical Torii MCP JSON-RPC contract (`/v2/mcp`) for agent/tool integrations.
