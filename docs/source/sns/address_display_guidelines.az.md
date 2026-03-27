# Sora Address Display Guidelines (ADDR-6)

Wallets, explorers, SDKs, and CLI samples must treat the canonical I105 literal as the only public account id format. On-chain aliases are a
separate lookup surface:

- `name@dataspace`
- `name@domain.dataspace`

Those aliases resolve on-chain to a canonical i105 account id. They are not an
alternate account-id encoding, and they must never be presented as if they were
an omitted domain suffix on the i105 literal.

## Required UX

- **Copy/share only canonical i105.** Ship one primary copy action for the
  canonical I105 account id. The same literal powers QR payloads,
  deep links, and clipboard actions.
- **Render aliases as secondary metadata.** When a workflow has an on-chain
  alias, show it in a separate labeled field such as “Alias” or “Routing
  alias”. Do not concatenate it onto the i105 literal.
- **Use monospace, selectable text.** Render the i105 literal in a selectable
  monospace field so users can inspect it without triggering IME rewrites.
- **Confirm the exact copied value.** Copy toasts/snackbars should quote the
  canonical i105 literal so users can verify that the checksummed account id
  was copied.
- **Never expose alternate public account-id encodings.** Canonical hex and
  legacy/non-canonical address forms are tooling/debug inputs only. Production
  wallet and explorer surfaces must not market them as copy/share formats.

Following these guardrails prevents Unicode/IME corruption and keeps every
surface aligned with the hard-cut parser contract.

## Screenshot fixtures

Use the following fixtures during localization reviews to ensure button labels,
tooltips, and QR captions stay aligned across platforms:

- Android reference: `images/address_copy_android.svg`

  ![Android address copy reference](images/address_copy_android.svg)

- iOS reference: `images/address_copy_ios.svg`

  ![iOS address copy reference](images/address_copy_ios.svg)

## SDK helpers

Each SDK exposes a helper that returns canonical I105 rendering plus
warning text so UI layers do not have to reimplement the codec:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)` (`javascript/iroha_js/src/address.js`)
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Swift: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)` (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Use these helpers for rendering. Use alias-aware Torii endpoints when you need
to resolve `name@dataspace` or `name@domain.dataspace` into the canonical i105
account id.

## Torii API contract

- Strict `AccountId` parser paths accept only canonical I105.
- Alias-aware routes may additionally accept on-chain aliases in
  `name@dataspace` or `name@domain.dataspace` form.
- Responses render canonical I105 account ids even when the caller
  used an alias as the lookup key.
- Clients must not append `@<domain>` suffixes to i105 literals.

This contract applies consistently to explorer/account endpoints, offline
reserve flows, manifests, telemetry payloads, and QR generation.

## Accessibility + alias metadata

- **Distinct controls.** Mark the copy/share control as “Copy canonical i105
  account id” or “Share i105 QR”. If you also offer alias copy, label it
  explicitly as alias copy/view.
- **Separate descriptions.** Put alias or domain context in `aria-describedby`
  or adjacent helper text, not inside the i105 field.
- **QR alt text.** Describe the QR as an i105 account id for the resolved
  account, with alias context called out separately when present.
- **Telemetry hooks.** Emit copy-mode telemetry such as `i105`, `alias`, and
  `qr` so UX reviews can prove canonical i105 remains the primary share flow.

## Tooling checks

1. Run `iroha tools address convert <literal> --format json` when auditing an
   account literal. Strict account-id paths should accept canonical i105 only.
2. Use alias-aware APIs to resolve `name@dataspace` or
   `name@domain.dataspace`; do not feed aliases into strict `AccountId`
   parsers.
3. Block releases when UI tests show missing ARIA labels, truncated QR text, or
   non-canonical account-id copy behaviour.
   The helper preserves the i105 prefix detected from the literal unless you provide
   `networkPrefix`, so summaries for non-default networks do not silently re-render with the
   default prefix.

3. Convert the canonical payload by reusing the `i105.value` or `canonicalHex` from the summary
   (or request another encoding via `--format`). These strings are already safe to share externally.
4. Update manifests, registries, and customer-facing documents with the canonical form and notify
   counterparties that Local selectors will be rejected once the cutover completes.
5. For bulk data sets, run `iroha tools address audit --input addresses.txt --network-prefix 753`.
   The command reads newline-separated literals (comments starting with `#` are ignored, and
   `--input -` or no flag uses STDIN) and emits JSON or CSV summaries for every entry. By default
   audit fails on parse errors; use `--allow-errors` only for best-effort scans.
6. For newline-to-newline rewrites, run `iroha tools address normalize --input addresses.txt --network-prefix 753 --format i105`.
   The helper rewrites each parsed row into the requested encoding
   (canonical I105/hex/JSON). Pair it with `--allow-errors` to keep scanning
   malformed dumps.
7. CI/lint automation can run `ci/check_address_normalize.sh`, which extracts the Local selectors from
   `fixtures/account/address_vectors.json`, converts them via `iroha tools address normalize`, and audits
   the result to ensure parse errors are zero and no `domain.kind="local12"` entries remain.

`torii_address_local8_total{endpoint}` plus
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, and the Grafana board
`dashboards/grafana/address_ingest.json` provide the enforcement signal: once production dashboards
show zero legitimate Local submissions and zero Local-12 collisions for 30 consecutive days, Torii
will flip the Local-8 gate to hard-fail on mainnet, followed by Local-12 once global domains have
matching registry entries.
Consider the CLI output the operator-facing notice for this freeze. The accompanying
Alertmanager pack (`dashboards/alerts/address_ingest_rules.yml`) surfaces three guardrails:

- `AddressLocal8Resurgence` pages whenever any context reports a fresh Local-8 increment. Treat this
  as a release blocker—identify the offending SDK surface via the dashboards, ship a fix, and keep
  the gate closed until the signal returns to zero.
- `AddressLocal12Collision` fires when two Local-12 labels hash to the same digest. Pause manifest
  promotions, run the Local → Global toolkit to audit the digest mapping, and coordinate with Nexus
  governance before reissuing the registry entry or re-enabling downstream rollouts.
- `AddressInvalidRatioSlo` warns when the fleet-wide invalid ratio exceeds the 0.1 % SLO for ten
  minutes. Investigate `torii_address_invalid_total` by reason/context and coordinate with the
  responsible SDK team before declaring the incident resolved.

`torii_address_format_total{endpoint,format}` complements the ingest metrics by counting every
`canonical I105 literal rendering` request that Torii serves. Dashboard the metric alongside
`torii_address_invalid_total` to prove that wallet/explorer traffic is gradually switching to the
i105 output before you disable Local selectors, and wire alert thresholds to catch any sudden
fallback to the default i105 responses.

### Release note snippet (wallet & explorer)

Include the following bullet in the wallet/explorer release notes when shipping the cutover:

> **Addresses:** Updated migration tooling to use canonical address literals only.
> `ci/check_address_normalize.sh` now normalizes fixture rows with
> `iroha tools address normalize` and blocks releases when audit reports parse
> errors or residual `domain.kind="local12"` entries.
