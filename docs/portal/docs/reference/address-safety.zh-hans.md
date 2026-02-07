---
lang: zh-hans
direction: ltr
source: docs/portal/docs/reference/address-safety.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d19690fde1b9e3c6f072cf1ac5ad89fb82578eb1b21e4d8dbc82d399518b4d42
source_last_modified: "2026-01-28T17:11:30.641899+00:00"
translation_last_reviewed: 2026-02-07
title: Address Safety & Accessibility
description: UX requirements for presenting and sharing Iroha addresses safely (ADDR-6c).
---

This page captures the ADDR-6c documentation deliverable. Apply these
constraints to wallets, explorers, SDK tooling, and any portal surface that
renders or accepts human-facing addresses. The canonical data model lives in
`docs/account_structure.md`; the checklist below explains how to expose those
formats without compromising safety or accessibility.

## Safe sharing flows

- Default every copy/share action to the IH58 address. Display the resolved
  domain as supporting context so the checksummed string stays front and centre.
- Offer a “Share” affordance that bundles the full plain-text address and a QR
  code derived from the same payload. Let users inspect both before committing.
- When space requires truncation (tiny cards, notifications), keep the leading
  human-readable prefix, show ellipses, and retain the final 4–6 characters so
  the checksum anchor survives. Provide a tap/keyboard shortcut to copy the full
  string without truncation.
- Prevent clipboard desync by emitting a confirmation toast that previews the
  exact IH58 string that was copied. Where telemetry is available, count copy
  attempts versus share actions so UX regressions surface quickly.

## IME & input safeguards

- Reject non-ASCII input in address fields. When IME composition artefacts (full
  width, Kana, tone marks) appear, surface an inline warning that explains how
  to switch the keyboard to Latin input before retrying.
- Provide a plain-text paste zone that strips combining marks and replaces
  whitespace with ASCII spaces before validation. This keeps users from losing
  progress when they disable their IME mid-flow.
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
- Supply descriptive `alt` text for QR previews (e.g., “IH58 address for
  `<account>` on chain `0x1234`”). Provide a “Copy address as text”
  fallback adjacent to the QR canvas for low-vision users.

## Sora-only compressed addresses

- Gating: hide the `sora…` compressed string behind an explicit confirmation.
  The confirmation must reiterate that the form only works on Sora Nexus chains.
- Labelling: every occurrence must include a visible “Sora-only” badge and a
  tooltip describing why other networks require the IH58 form.
- Guardrails: if the active chain discriminant is not the Nexus allocation,
  refuse to generate the compressed address entirely and direct the user back to
  IH58.
- Telemetry: record how often the compressed form is requested and copied so the
  incident playbook can detect accidental sharing spikes.

## Quality gates

- Extend automated UI tests (or storybook a11y suites) to assert that address
  components expose the required ARIA metadata and that IME rejection messages
  appear.
- Include manual QA scenarios for IME input (kana, pinyin), screen reader pass
  (VoiceOver/NVDA), and QR copy on high-contrast themes before releasing.
- Surface these checks in release checklists alongside the IH58 parity tests
  so regressions remain blocked until corrected.
