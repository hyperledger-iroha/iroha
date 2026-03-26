---
id: address-display-guidelines
title: Sora Address Display Guidelines
sidebar_label: Address display
description: UX and CLI requirements for I105 vs I105 Sora address presentation (ADDR-6).
---

import ExplorerAddressCard from '@site/src/components/ExplorerAddressCard';

:::note Canonical Source
This page mirrors `docs/source/sns/address_display_guidelines.md` and now serves
as the canonical portal copy. The source file sticks around for translation PRs.
:::

Wallets, explorers, and SDK samples must treat account addresses as immutable
payloads. The Android retail wallet sample in
`examples/android/retail-wallet` now demonstrates the required UX pattern:

- **Dual copy targets.** Ship two explicit copy buttons—I105 and the
  i105-default Sora-only form (`i105`). I105 is always safe to share externally
  and powers the QR payload. The i105-default variant must include an inline
  warning because it only works inside Sora-aware apps. The Android retail
  wallet sample wires both Material buttons and their tooltips in
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, and
  the iOS SwiftUI demo mirrors the same UX via `AddressPreviewCard` inside
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace, selectable text.** Render both strings with a monospace font and
  `textIsSelectable="true"` so users can inspect values without invoking an IME.
  Avoid editable fields: IMEs can rewrite kana or inject zero-width code points.
- **Domainless address hints.** Canonical account literals are domainless; when a workflow needs domain context, render it separately from the account literal. When the selector points at the implicit
  `default` domain, surface a caption reminding operators no suffix is required.
  Explorers should also highlight the canonical domain label when the selector
  encodes a digest.
- **I105 QR payloads.** QR codes must encode the I105 string. If QR generation
  fails, display an explicit error instead of a blank image.
- **Clipboard messaging.** After copying the i105-default form, emit a toast or
  snackbar reminding users that it is Sora-only and prone to IME mangling.

Following these guardrails prevents Unicode/IME corruption and satisfies the
ADDR-6 roadmap acceptance criteria for wallet/explorer UX.

## Screenshot fixtures

Use the following fixtures during localization reviews to ensure button labels,
tooltips, and warnings stay aligned across platforms:

- Android reference: `/img/sns/address_copy_android.svg`

  ![Android dual copy reference](/img/sns/address_copy_android.svg)

- iOS reference: `/img/sns/address_copy_ios.svg`

  ![iOS dual copy reference](/img/sns/address_copy_ios.svg)

## SDK helpers

Each SDK exposes a convenience helper that returns the I105
forms alongside the warning string so UI layers can stay consistent:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspector: `inspectAccountId(...)` returns the i105-default warning
  string and appends it to `warnings` whenever callers provide a `i105`
  literal, so explorers/wallet dashboards can surface the Sora-only notice
  during paste/validation flows instead of only when they generate the
  i105-default form themselves.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Swift: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Use these helpers instead of reimplementing the encode logic in UI layers.
The JavaScript helper also exposes a `selector` payload on `domainSummary`
(`tag`, `digest_hex`, `registry_id`, `label`) so UIs can indicate whether a
selector is Local-12 or registry-backed without re-parsing the raw payload.

## Explorer instrumentation demo

<ExplorerAddressCard />

Explorers should mirror the wallet telemetry and accessibility work:

- Apply `data-copy-mode="i105|i105_default|qr"` to copy buttons so front-ends can emit usage counters
  alongside the Torii-side `torii_address_format_total` metric. The demo component above dispatches
  an `iroha:address-copy` event with `{mode,timestamp}`—wire this into your analytics/telemetry
  pipeline (e.g., push to Segment or a NORITO-backed collector) so dashboards can correlate server
  address-format usage with client copy behaviour. Also mirror the Torii domain counters
  (`torii_address_domain_total{domain_kind}`) in the same feed so Local-12 retirement reviews can
  export a 30-day `domain_kind="local12"` zero-usage proof directly from the `address_ingest`
  Grafana board.
- Pair every control with distinct `aria-label`/`aria-describedby` hints that explain whether a
  literal is safe to share (`I105`) or Sora-only (i105-default `sora`). Include the implicit-domain caption in
  the description so assistive technology surfaces the same context shown visually.
- Expose a live region (e.g., `<output aria-live="polite">…</output>`) announcing copy results and
  warnings, matching the VoiceOver/TalkBack behaviour now wired into the Swift/Android samples.

This instrumentation satisfies ADDR-6b by proving operators can observe both Torii ingestion and
client-side copy modes before Local selectors are disabled.

## Local → Global migration toolkit

Use the [Local → Global toolkit](local-to-global-toolkit.md) to automate
JSON audit report and the converted preferred I105 list that operators attach
to readiness tickets, while the accompanying runbook links the Grafana
dashboards and Alertmanager rules that gate the strict-mode cutover.

## Binary layout quick reference (ADDR-1a)

When SDKs surface advanced address tooling (inspectors, validation hints,
manifest builders), point developers at the canonical wire format captured in
`docs/account_structure.md`. The layout is always
`header · selector · controller`, where the header bits are:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7‑5) today; non-zero values are reserved and must
  raise `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` distinguishes single (`0`) vs multisig (`1`) controllers.
- `norm_version = 1` encodes the Norm v1 selector rules. Future norms will reuse
  the same 2-bit field.
- `ext_flag` is always `0`—set bits indicate unsupported payload extensions.

The selector immediately follows the header:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UI and SDK surfaces should be ready to display the selector kind:

- `0x00` = implicit default domain (no payload).
- `0x01` = local digest (12-byte `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = global registry entry (big-endian `registry_id:u32`).

Canonical hex examples that wallet tooling can link or embed in docs/tests:

| Selector kind | Canonical hex |
|---------------|---------------|
| Implicit default | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Local digest (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Global registry (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Refer to `docs/source/references/address_norm_v1.md` for the full selector/state
table and `docs/account_structure.md` for the complete byte diagram.

## Enforcing canonical forms

Use the CLI workflow documented under ADDR-5:

1. Run `iroha tools address convert <address-or-account_id> --format json`.
   The summary reports `detected_format`, `domain.kind`, and canonical
   encodings (`i105`, `i105_default`, `canonical_hex`). Inputs must be canonical
   address literals (canonical i105 only on strict parser paths); i105-default `sora...`, canonical hex, and `@<domain>`
   suffixes are rejected on strict parser paths.
2. SDKs can surface the same summary via JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.kind === "local12") {
     console.warn("Local selector detected; migrate to canonical global address.");
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
3. Reuse `i105.value` or `i105_default` from the summary (or request another
   encoding via `--format`) in manifests and user-facing docs.
4. For bulk data sets, run
   `iroha tools address audit --input addresses.txt --network-prefix 753`.
   Audit emits JSON/CSV summaries per row and fails on parse errors by default.
   Use `--allow-errors` only for best-effort scans.
5. For newline-to-newline rewrites, run
   `iroha tools address normalize --input addresses.txt --network-prefix 753 --format i105`.
   This rewrites each parsed row to the requested encoding
   (canonical i105/hex/JSON). Pair with
   `--allow-errors` for malformed dump triage.
6. CI/lint automation can run `ci/check_address_normalize.sh`, which extracts
   Local-selector fixture rows, normalizes them, then audits to ensure zero parse
   errors and no residual `domain.kind="local12"` entries.

`torii_address_local8_total{endpoint}`,
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, and the Grafana board
`dashboards/grafana/address_ingest.json` provide the enforcement signal: once
production dashboards show zero legitimate Local submissions and zero Local-12
collisions for 30 consecutive days, Torii flips Local-8/Local-12 acceptance to
hard-fail according to rollout policy. The Alertmanager pack
(`dashboards/alerts/address_ingest_rules.yml`) adds three guardrails:

- `AddressLocal8Resurgence` pages on any fresh Local-8 increment.
- `AddressLocal12Collision` pages on Local-12 digest collisions.
- `AddressInvalidRatioSlo` warns when invalid payload ratio exceeds the 0.1% SLO.

### Release note snippet (wallet & explorer)

Include the following bullet in wallet/explorer release notes when shipping the cutover:

> **Addresses:** Migration tooling now uses canonical address literals only.
> `ci/check_address_normalize.sh` normalizes fixture rows with
> `iroha tools address normalize` and blocks releases when audit reports parse
> errors or residual `domain.kind="local12"` entries.
