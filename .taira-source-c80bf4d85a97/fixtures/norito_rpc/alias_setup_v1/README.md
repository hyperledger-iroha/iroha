# Alias setup V1 shared fixtures

`alias_setup_v1.json` is consumed by the Rust, Kotlin, mirrored Java, and Swift
alias suites. It pins catalog-free and resolved names, a guarded quote, exact
alias permission scope, genuine Rust-produced V1 instruction frames, setup and
lifecycle plan commitments, one domain-separated signed sponsored-onboarding
receipt, and a secret-free blocked readiness report.

The `account_onboarding_receipt_vector` is produced by the Rust data-model and
crypto implementations. It pins the exact bare Norito receipt body, its
domain-separated hash, the configured authority and Ed25519 signature, and the
typed Torii transport JSON consumed by every SDK suite.

The instruction vectors cover:

- `EnsureAlias` for an account alias;
- `RenewAliasLease`;
- `ConfigureAliasAutoRenew` enable and disable;
- `RebindAccountAlias`; and
- `CompareAndSetPrimaryAccountAlias`.

Each `framed_payload_hex` is the complete canonical Norito archive, including
the `NRT0` header, schema hash, payload length, CRC64, advertised layout flags,
alignment padding, and typed payload. The plan vectors contain the exact bare
Norito body bytes hashed by Rust with the recorded domain separator. The
readiness report uses the same tagged JSON layout as `AliasSetupReportV1`. The
compact JSON vectors additionally pin `ResolvedDataSpaceV1`,
`ResolvedDomainV1`, `ResolvedAccountAliasV1`, `AliasQuoteGuardV1`, and
`AccountAliasPermissionScope::Alias` across all four SDK suites.

Rust, Kotlin, and mirrored Java perform a typed registry decode and canonical
re-encode of every frame. Swift performs the same validation through the native
Rust instruction-registry bridge and fails closed if the required bridge symbol
is unavailable. All four SDK suites decode or bridge-encode the exact setup,
lifecycle, and sponsored-onboarding plan bodies committed by the recorded
hashes; the onboarding vector also pins the authority signature and optional
configured-authority check.

When a V1 layout changes intentionally, regenerate every frame and plan body
from the Rust data-model types, then update all four fixture tests together.
