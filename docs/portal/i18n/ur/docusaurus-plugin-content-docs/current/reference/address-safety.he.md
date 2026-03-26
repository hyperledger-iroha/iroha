---
title: Address Safety & Accessibility
description: UX requirements for presenting and sharing Iroha addresses safely (ADDR-6c).
---

This page captures the ADDR-6c documentation deliverable. Apply these
constraints to wallets, explorers, SDK tooling, and any portal surface that
renders or accepts human-facing addresses. The canonical data model lives in
`docs/account_structure.md`; the checklist below explains how to expose those
formats without compromising safety or accessibility.

## Safe sharing flows

- Default every copy/share action to the canonical Katakana i105 account id.
  If an on-chain alias is present, display it as supporting metadata in a
  separate labeled field.
- Offer a “Share” affordance that bundles the full plain-text address and a QR
  code derived from the same payload. Let users inspect both before committing.
- When space requires truncation (tiny cards, notifications), keep the leading
  human-readable prefix, show ellipses, and retain the final 4–6 characters so
  the checksum anchor survives. Provide a tap/keyboard shortcut to copy the full
  string without truncation.
- Prevent clipboard desync by emitting a confirmation toast that previews the
  exact i105 string that was copied. Where telemetry is available, count copy
  attempts versus share actions so UX regressions surface quickly.

## IME & input safeguards

- Validate account-id fields as canonical Katakana i105 only. Validate alias
  entry fields separately as `name@dataspace` or `name@domain.dataspace`.
- When IME composition artefacts or zero-width characters appear, surface an
  inline warning instead of coercing the input into a different account-id
  format.
- Provide a plain-text paste zone that preserves the canonical i105 literal as
  pasted while still stripping obviously invalid stealth code points before
  validation.
- Harden validation against zero-width joiners, variation selectors, and other
  stealth Unicode code points. Log the rejected code point category so fuzzing
  suites can import the telemetry.

## Assistive technology expectations

- Annotate every address block with `aria-label` or `aria-describedby` that
  spells out the human-readable prefix and chunks the payload in 4–8 character
  groups (“ih dash b three two …”). This stops screen readers from producing an
  unintelligible stream of characters.
- Announce successful copy/share events via a polite live region update. Include
  the destination (clipboard, share sheet, QR) so the user knows the action
  completed without moving focus.
- Supply descriptive `alt` text for QR previews (e.g., “i105 address for
  `<account>` on chain `0x1234`”). Provide a “Copy address as text”
  fallback adjacent to the QR canvas for low-vision users.

## Single-format policy

- Keep canonical Katakana i105 as the only user-facing account-id format for
  copy, share, and QR surfaces.
- Treat `name@dataspace` and `name@domain.dataspace` as on-chain aliases that
  point to canonical i105 account ids.
- Do not expose alternate account-literal encodings in production wallet or
  explorer UX.
- Telemetry should track i105 copy/share usage, alias-resolution usage, and
  validation failures only.

## Quality gates

- Extend automated UI tests (or storybook a11y suites) to assert that address
  components expose the required ARIA metadata and that IME rejection messages
  appear.
- Include manual QA scenarios for IME input (kana, pinyin), screen reader pass
  (VoiceOver/NVDA), and QR copy on high-contrast themes before releasing.
- Surface these checks in release checklists alongside the i105 parity tests
  so regressions remain blocked until corrected.
