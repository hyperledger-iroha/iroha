# Roadmap (Open Work Only)

Last updated: 2026-03-12

## aid/Alias Rollout
1. Add first-class alias indexes in world state (`alias -> aid`, `aid -> alias binding`) instead of linear scans.
2. Add dedicated alias bind/update instruction path for existing asset definitions and wire it through executor validation.
3. Add server-side alias resolution query/helper so CLI/SDK do not need client-side full-scan fallback.

## Alias Lease Lifecycle
1. Model alias lease expiry/grace on-chain and persist lifecycle fields with bindings.
2. Add time-trigger sweep to unbind aliases after `lease_expiry + 369h` grace.
3. Add integration coverage for in-grace resolution and post-grace unbind behavior.

## Docs/Examples
1. Add dedicated docs for canonical `aid:<32-hex-no-dash>` + alias literal usage in CLI/Torii examples.
2. Update remaining examples that still imply manual Norito-only asset-id workflows.
