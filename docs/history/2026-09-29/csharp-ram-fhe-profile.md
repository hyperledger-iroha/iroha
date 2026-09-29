# C# RAM-FHE profile validation

`ToriiIdentifierPolicySummary.RamFheProfile` now carries an immutable
`ToriiRamFheProfile` instead of an untyped JSON tree. The seven mandatory fields
use positive `byte`, `ushort` and `ulong` values, the sole encrypted-envelope
mode, and an exact 64-character lowercase initializer hash with the Iroha marker
bit set. Unknown, missing, duplicate, null, coerced numeric and malformed fields
are rejected. Optional absence of the whole policy profile remains explicit.
No route, compatibility decoder, or execution-proof activation is introduced.

Normal .NET SDK 8.0.419 builds with zero warnings/errors. Eleven dedicated
profile tests plus the existing identifier receipt controls pass together:
154 tests, zero failures/skips, 0.441 seconds. The existing client JSON ownership
and identifier-policy route controls separately pass: two tests, zero failures/
skips, 0.390 seconds. The dedicated transport test exercises the actual client
`GET /v1/identifier-policies` converter using the SDK's generated JSON metadata.
It tests both a valid typed result and a malformed hash with no fallback.

The first run exposed the new type's missing generated metadata registration;
`ToriiJsonSerializerContext` now declares it. Three other first-run failures
came from invoking existing authenticated account tests without the required
native library search path. The final run uses the ordinary platform loader
with the retained ABI-24 bridge, without an injected resolver or managed
substitute. This is host SDK validation, not a fresh bridge or device/release
qualification.

Exact sources, binaries, original failed logs, passing logs, normal loader
artifact hash, scoped format result and codec guard are recorded in
`dist/zk-remediation/2026-09-29/csharp-ram-fhe-profile/complete-receipt.json`.
The pinned SDK was downloaded from Microsoft's official release metadata and
verified against its SHA-512 digest; the repository's `global.json` was unchanged.
