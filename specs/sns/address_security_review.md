<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Account Address Security Review (ADDR-7)

Shipping V1 account identity is universal and domainless. An account address
contains only its version/header and controller payload; domains and SNS labels
are separate routing or alias records and are never encoded into `AccountId`.
Selector-bearing formats are outside the first-release protocol.

## Parser and codec invariants

- Public account-id inputs use canonical I105. Strict runtime parsers reject
  aliases, `@domain` suffixes, public-key literals, canonical-hex literals, and
  non-canonical I105 spellings.
- Binary decoding consumes the exact canonical header/controller payload.
  Selector-prefixed, truncated, or trailing bytes are rejected instead of
  being interpreted as another address version.
- The I105 checksum is verified before an address is admitted. Network-prefix
  and chain-discriminant checks fail closed when a caller supplies an expected
  value.
- Multisignature controllers validate member ordering, weights, quorum, and
  payload bounds before construction succeeds.
- Alias-aware routes resolve `name@dataspace` or
  `name@domain.dataspace` explicitly, then return the resolved canonical
  account id. They do not relax strict `AccountId` parsing.

The normative binary and textual cases live in
`fixtures/account/address_vectors.json`. Regenerate or verify them with
`cargo xtask address-vectors`; Rust and SDK fixture suites must consume the
same selector-free fixture.

## Normalization and display controls

Domain and alias labels use the Norm v1 spelling and UTS-46 checks documented
in [`address_norm_v1.md`](../references/address_norm_v1.md). Normalization is
performed on the separate label record, never on account-address bytes.

Wallets and explorers must expose one primary copy/share representation: the
canonical I105 account id. Aliases are separately labeled metadata. See
[`address_display_guidelines.md`](./address_display_guidelines.md) for the UX
contract and checksum/IME safeguards.

## Operational controls

- Torii records rejected literals in
  `torii_address_invalid_total{endpoint,reason}`. The reason is a bounded,
  stable parser error code; raw account input is never a metric label.
- `AddressInvalidRatioSlo` and
  `dashboards/grafana/address_ingest.json` expose invalid rates and top failure
  reasons. There are no selector-kind or digest-collision counters.
- A release is blocked when the canonical fixture drifts, an SDK accepts a
  forbidden account-id form, or strict parser negative cases stop failing.
- Checksum incidents follow
  [`address_checksum_failure_runbook.md`](./address_checksum_failure_runbook.md).

## Review checklist

1. Run `cargo xtask address-vectors --verify` and the affected Rust/SDK fixture
   suites.
2. Confirm strict paths accept canonical I105 and reject aliases, suffixes,
   hex, public-key literals, selector-prefixed bytes, and malformed checksums.
3. Confirm alias-aware paths resolve through explicit on-chain alias state and
   return a canonical domainless account id.
4. Review the invalid-address dashboard without retaining raw user input.
5. Attach the fixture digest and focused test results to the release record.
