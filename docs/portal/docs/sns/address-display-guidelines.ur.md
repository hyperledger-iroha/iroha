---
id: address-display-guidelines
title: Sora Address Display Guidelines
sidebar_label: Address display
description: UX and CLI requirements for canonical i105 account ids and on-chain aliases (ADDR-6).
---

import ExplorerAddressCard from '@site/src/components/ExplorerAddressCard';

:::note Canonical Source
This page mirrors `docs/source/sns/address_display_guidelines.md` and now serves
as the canonical portal copy. The source file sticks around for translation PRs.
:::

Wallets, explorers, and SDK samples must treat the canonical Katakana i105
literal as the only public account-id format. On-chain aliases are separate
lookup keys:

- `name@dataspace`
- `name@domain.dataspace`

Those aliases resolve on-chain to the canonical i105 account id. They are not
an alternate public account-id encoding.

## Required UX

- **Copy/share only canonical i105.** Ship one primary copy action for the
  canonical Katakana i105 account id. That same literal powers QR payloads,
  deep links, and clipboard actions.
- **Render aliases separately.** If a workflow includes an alias, show it in a
  labeled field such as “Alias” or “Routing alias”. Do not concatenate it onto
  the i105 literal.
- **Monospace, selectable text.** Render the i105 literal with a monospace font
  and `textIsSelectable="true"` so users can inspect it without invoking an
  IME. Avoid editable fields: IMEs can rewrite kana or inject zero-width code
  points.
- **Confirm the exact copied value.** After copying the i105 form, emit a toast
  or snackbar that quotes the canonical i105 literal.
- **No alternate public encodings.** Canonical hex and legacy/non-canonical
  address forms are tooling/debug inputs only and must not be marketed as
  copy/share formats.

## Screenshot fixtures

Use the following fixtures during localization reviews to ensure button labels,
tooltips, and warnings stay aligned across platforms:

- Android reference: `/img/sns/address_copy_android.svg`

  ![Android address copy reference](/img/sns/address_copy_android.svg)

- iOS reference: `/img/sns/address_copy_ios.svg`

  ![iOS address copy reference](/img/sns/address_copy_ios.svg)

## SDK helpers

Each SDK exposes a convenience helper that returns canonical Katakana i105
rendering plus warning text so UI layers can stay consistent:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Swift: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Use these helpers instead of reimplementing the encode logic in UI layers.
Use alias-aware Torii endpoints when you need to resolve `name@dataspace` or
`name@domain.dataspace` into the canonical i105 account id.

## Explorer instrumentation demo

<ExplorerAddressCard />

Explorers should mirror the wallet telemetry and accessibility work:

- Apply `data-copy-mode="i105|alias|qr"` to copy buttons so front-ends can emit
  usage counters alongside server-side account-literal metrics.
- Pair every control with distinct `aria-label`/`aria-describedby` hints that
  explain whether a control copies the canonical i105 account id, views the
  alias, or shares the QR payload.
- Expose a live region (e.g., `<output aria-live="polite">…</output>`) announcing copy results and
  warnings, matching the VoiceOver/TalkBack behaviour now wired into the Swift/Android samples.

## Enforcing canonical forms

Use the CLI workflow documented under ADDR-5:

1. Run `iroha tools address convert <address-or-account_id> --format json`.
   The summary reports the detected format and canonical encodings (`i105`,
   `canonical_hex`). Strict parser paths accept canonical Katakana i105 only.
2. SDKs can surface the same summary via JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId(accountLiteral);
   console.log(summary.i105.value, summary.i105Warning);
   ```
3. Resolve aliases through alias-aware APIs instead of feeding
   `name@dataspace` or `name@domain.dataspace` into strict `AccountId`
   parsers.
4. Reuse `i105.value` from the summary (or request another encoding via
   `--format`) in manifests and user-facing docs.
4. For bulk data sets, run
   `iroha tools address audit --input addresses.txt --network-prefix 753`.
   Audit emits JSON/CSV summaries per row and fails on parse errors by default.
   Use `--allow-errors` only for best-effort scans.
5. For newline-to-newline rewrites, run
   `iroha tools address normalize --input addresses.txt --network-prefix 753 --format i105`.
   This rewrites each parsed row to the requested encoding
   (canonical Katakana i105/hex/JSON). Pair with
   `--allow-errors` for malformed dump triage.

### Release note snippet (wallet & explorer)

Include the following bullet in wallet/explorer release notes when shipping the cutover:

> **Addresses:** Copy/share flows now use canonical Katakana i105 account ids
> only. On-chain aliases remain available as separate routing labels in
> `name@dataspace` / `name@domain.dataspace` form and resolve to the same
> canonical i105 account id.
