---
id: address-display-guidelines
title: Sora Address Display Guidelines
sidebar_label: Address display
description: UX and CLI requirements for IH58 vs compressed (`sora`) Sora address presentation (ADDR-6).
---

import ExplorerAddressCard from '@site/src/components/ExplorerAddressCard';

:::note Canonical Source
This page mirrors `docs/source/sns/address_display_guidelines.md` and now serves
as the canonical portal copy. The source file sticks around for translation PRs.
:::

Wallets, explorers, and SDK samples must treat account addresses as immutable
payloads. The Android retail wallet sample in
`examples/android/retail-wallet` now demonstrates the required UX pattern:

- **Dual copy targets.** Ship two explicit copy buttons—IH58 (preferred) and the
  compressed Sora-only form (`sora…`, second-best). IH58 is always safe to share externally
  and powers the QR payload. The compressed variant must include an inline
  warning because it only works inside Sora-aware apps. The Android retail
  wallet sample wires both Material buttons and their tooltips in
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, and
  the iOS SwiftUI demo mirrors the same UX via `AddressPreviewCard` inside
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace, selectable text.** Render both strings with a monospace font and
  `textIsSelectable="true"` so users can inspect values without invoking an IME.
  Avoid editable fields: IMEs can rewrite kana or inject zero-width code points.
- **Implicit default domain hints.** When the selector points at the implicit
  `default` domain, surface a caption reminding operators no suffix is required.
  Explorers should also highlight the canonical domain label when the selector
  encodes a digest.
- **IH58 QR payloads.** QR codes must encode the IH58 string. If QR generation
  fails, display an explicit error instead of a blank image.
- **Clipboard messaging.** After copying the compressed form, emit a toast or
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

Each SDK exposes a convenience helper that returns the IH58 (preferred) and compressed (`sora`, second-best)
forms alongside the warning string so UI layers can stay consistent:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspector: `inspectAccountId(...)` returns the compressed warning
  string and appends it to `warnings` whenever callers provide a `sora…`
  literal, so explorers/wallet dashboards can surface the Sora-only notice
  during paste/validation flows instead of only when they generate the
  compressed form themselves.
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

- Apply `data-copy-mode="ih58|compressed|qr"` to copy buttons so front-ends can emit usage counters
  alongside the Torii-side `torii_address_format_total` metric. The demo component above dispatches
  an `iroha:address-copy` event with `{mode,timestamp}`—wire this into your analytics/telemetry
  pipeline (e.g., push to Segment or a NORITO-backed collector) so dashboards can correlate server
  address-format usage with client copy behaviour. Also mirror the Torii domain counters
  (`torii_address_domain_total{domain_kind}`) in the same feed so Local-12 retirement reviews can
  export a 30-day `domain_kind="local12"` zero-usage proof directly from the `address_ingest`
  Grafana board.
- Pair every control with distinct `aria-label`/`aria-describedby` hints that explain whether a
  literal is safe to share (`IH58`) or Sora-only (compressed `sora`). Include the implicit-domain caption in
  the description so assistive technology surfaces the same context shown visually.
- Expose a live region (e.g., `<output aria-live="polite">…</output>`) announcing copy results and
  warnings, matching the VoiceOver/TalkBack behaviour now wired into the Swift/Android samples.

This instrumentation satisfies ADDR-6b by proving operators can observe both Torii ingestion and
client-side copy modes before Local selectors are disabled.

## Local → Global migration toolkit

Use the [Local → Global toolkit](local-to-global-toolkit.md) to automate
JSON audit report and the converted preferred IH58 / second-best compressed (`sora`) list that operators attach
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
| Implicit default | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Local digest (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| Global registry (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Refer to `docs/source/references/address_norm_v1.md` for the full selector/state
table and `docs/account_structure.md` for the complete byte diagram.

## Enforcing canonical forms

strings must follow the CLI workflow documented under ADDR-5:

1. `iroha tools address inspect` now emits a structured JSON summary with IH58,
   compressed, and canonical hex payloads. The summary also includes a `domain`
   object with `kind`/`warning` fields and echoes any provided domain via the
   `input_domain` field. When `kind` is `local12`, the CLI prints a warning to
   stderr and the JSON summary echoes the same guidance so CI pipelines and SDKs
   can surface it. Pass `--append-domain` whenever you want the converted
   encoding replayed as `<ih58>@<domain>`.
2. SDKs can surface the same warning/summary via the JavaScript helper:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  The helper preserves the IH58 prefix detected from the literal unless you
  explicitly provide `networkPrefix`, so summaries for non-default networks do
  not silently re-render with the default prefix.

3. Convert the canonical payload by reusing the `ih58.value` or `compressed`
   fields from the summary (or request another encoding via `--format`). These
   strings are already safe to share externally.
4. Update manifests, registries, and customer-facing documents with the
   canonical form and notify counterparties that Local selectors will be
   rejected once the cutover completes.
5. For bulk data sets, run
   `iroha tools address audit --input addresses.txt --network-prefix 753`. The command
   reads newline-separated literals (comments starting with `#` are ignored, and
   `--input -` or no flag uses STDIN), emits a JSON report with
   canonical/preferred IH58/second-best compressed (`sora`) summaries for every entry, and counts both parse
   dumps that contain junk rows, and gate automation with `--fail-on-warning`
   once operators are ready to block Local selectors in CI.
6. When you need a newline-to-newline rewrite, use
  For Local-selector remediation spreadsheets, use
  to export a `input,status,format,…` CSV that highlights canonical encodings, warnings, and parse failures in one pass.
   The helper skips non-Local rows by default, converts every remaining entry
   into the requested encoding (IH58 preferred/compressed (`sora`) second-best/hex/JSON), and preserves the
   original domain when `--append-domain` is set. Pair it with `--allow-errors`
   to keep scanning even when a dump contains malformed literals.
7. CI/lint automation can run `ci/check_address_normalize.sh`, which extracts
   the Local selectors from `fixtures/account/address_vectors.json`, converts
   them via `iroha tools address normalize`, and replays
   `iroha tools address audit --fail-on-warning` to prove releases no longer emit
   Local digests.

`torii_address_local8_total{endpoint}` plus
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, and the
Grafana board `dashboards/grafana/address_ingest.json` provide the enforcement
signal: once production dashboards show zero legitimate Local submissions and
zero Local-12 collisions for 30 consecutive days, Torii will flip the Local-8
gate to hard-fail on mainnet, followed by Local-12 once global domains have
matching registry entries. Consider the CLI output the operator-facing notice
for this freeze—the same warning string is used across SDK tooltips and
automation to keep parity with the roadmap exit criteria. Torii now defaults to
when diagnosing regressions. Keep mirroring `torii_address_domain_total{domain_kind}`
into Grafana (`dashboards/grafana/address_ingest.json`) so the ADDR-7 evidence pack
can prove `domain_kind="local12"` stayed at zero for the required 30-day window before
(`dashboards/alerts/address_ingest_rules.yml`) adds three guardrails:

- `AddressLocal8Resurgence` pages whenever a context reports a fresh Local-8
  increment. Halt strict-mode rollouts, locate the offending SDK surface in the
  until the signal returns to zero—then restore the default (`true`).
- `AddressLocal12Collision` fires when two Local-12 labels hash to the same
  digest. Pause manifest promotions, run the Local → Global toolkit to audit
  the digest mapping, and coordinate with Nexus governance before reissuing the
  registry entry or re-enabling downstream rollouts.
- `AddressInvalidRatioSlo` warns when the fleet-wide invalid ratio (excluding
  Local-8/strict-mode rejections) exceeds the 0.1 % SLO for ten minutes. Use
  `torii_address_invalid_total` to pinpoint the responsible context/reason and
  coordinate with the owning SDK team before re-enabling strict mode.

### Release note snippet (wallet & explorer)

Include the following bullet in the wallet/explorer release notes when shipping
the cutover:

> **Addresses:** Added the `iroha tools address normalize --only-local --append-domain`
> helper and wired it into CI (`ci/check_address_normalize.sh`) so wallet/explorer
> before Local-8/Local-12 are blocked on mainnet. Update any custom exports to
> run the command and attach the normalized list to the release evidence bundle.
