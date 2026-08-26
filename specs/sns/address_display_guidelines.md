# Sora Address Display Guidelines (ADDR-6)

Wallets, explorers, SDKs, and CLI samples must treat canonical I105 as the only
public account-id format. On-chain aliases are a separate lookup surface:

- `name@dataspace`
- `name@domain.dataspace`

Those aliases resolve to a canonical domainless account id. They are not an
account-id encoding and must never be appended to an I105 literal.

## Required UX

- **Copy and share only canonical I105.** Use it for the primary copy action,
  QR payloads, deep links, and clipboard output.
- **Render aliases as secondary metadata.** Put aliases in a separately labeled
  field such as “Alias” or “Routing alias”.
- **Use monospace, selectable text.** Users must be able to inspect the literal
  without an IME rewriting it.
- **Confirm the exact copied value.** Copy feedback should quote the canonical
  I105 value.
- **Do not expose debug encodings.** Canonical hex is an operator/tooling view,
  not a public copy/share format.

## Torii contract

- Strict `AccountId` parser paths accept canonical I105 only.
- Alias-aware routes may additionally accept an on-chain alias.
- Responses render canonical I105 even when an alias was the lookup key.
- Clients must not append `@<domain>` to an I105 literal.
- Malformed checksums, non-canonical spellings, public-key literals, hex
  literals, and selector-prefixed payloads fail closed.

## Accessibility

- Label the primary action “Copy canonical I105 account id” or “Share I105
  QR”. Label any alias action separately.
- Keep alias or domain context in adjacent helper text or
  `aria-describedby`, never inside the account-id field.
- Describe a QR code as the canonical I105 account id for the resolved account;
  mention alias context separately when it is useful.
- Ensure localized layouts do not truncate the literal or silently substitute
  visually similar characters.

## Tooling checks

1. Use `iroha tools address convert <literal> --format json` for a single
   operator inspection. The JSON result contains canonical I105 and canonical
   hex, but no domain/selector classification.
2. Use `iroha tools address audit --input <path>` for newline-separated input.
   The command fails when a row cannot be parsed unless `--allow-errors` is
   supplied for a best-effort diagnostic run.
3. Use `iroha tools address normalize --input <path> --format i105` to rewrite
   successfully parsed rows into canonical I105.
4. Resolve aliases through alias-aware APIs; never feed aliases into a strict
   `AccountId` parser.
5. Block a release when UI tests find a non-I105 primary share value, missing
   accessibility labels, truncated QR text, or parser acceptance of a forbidden
   form.

The address-ingest dashboard and `AddressInvalidRatioSlo` alert track
`torii_address_invalid_total` by bounded failure reason. They are diagnostic
signals for malformed canonical input.
