<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Validation errors and structured diagnostics

The JS SDK raises `ValidationError` instances whenever builders, normalizers,
or codecs detect an invalid payload before issuing a Torii request. These
errors extend `TypeError` while exposing a stable `code` enum, the failing `path`,
and an optional `cause` so callers can surface actionable diagnostics without
string matching.

## Error model

- Exported as `ValidationError`/`ValidationErrorCode` from `iroha-js`.
- Instances keep `name = "ValidationError"` while remaining `instanceof TypeError`.
- `code` is a stable string enum useful for telemetry and user-facing messages.
- `path` identifies the offending field (e.g., `domainId`, `metadata.owner`).
- `cause` preserves nested errors (for example, when hex decoding fails).

## Error codes

- `ERR_INVALID_STRING` — empty or whitespace-only string input.
- `ERR_INVALID_HEX` — malformed or odd-length hexadecimal strings.
- `ERR_INVALID_MULTIHASH` — multihash parsing failed.
- `ERR_INVALID_ACCOUNT_ID` — account identifiers are not canonical I105 account literals. Domain suffixes (`@domain`), canonical hex, `uaid:`, and `opaque:` forms are rejected.
- `ERR_INVALID_ASSET_ID` — asset identifiers are not encoded `norito:<hex>` values.
- `ERR_INVALID_IBAN` — IBAN parsing/normalization failed (bad country code, length, or checksum).
- `ERR_INVALID_OBJECT` — unexpected object shape or missing required keys.
- `ERR_INVALID_METADATA` — metadata entries failed validation.
- `ERR_INVALID_JSON_VALUE` — JSON payloads are not well-formed.
- `ERR_INVALID_NUMERIC` — numeric parsing failed.
- `ERR_VALUE_OUT_OF_RANGE` — numeric bounds or domain-specific ranges violated.

## Domain suffixes

- Account IDs are domainless on SDK surfaces. Any `@domain` suffix is rejected with
  `ERR_INVALID_ACCOUNT_ID`.

## IBAN aliases

- ISO bridge aliases that resemble IBANs are normalized via `normalizeIban`, which uppercases
  input, strips whitespace, and validates country-specific lengths, numeric check digits,
  and the mod-97 checksum. Errors bubble as `ERR_INVALID_IBAN` with the input label recorded
  in `path` so clients can surface actionable prompts without text matching.

## Usage

```ts
import { ValidationError, ValidationErrorCode, normalizeAccountId } from "iroha-js";

try {
  normalizeAccountId("alice", "ownerId");
} catch (error) {
  if (error instanceof ValidationError) {
    if (error.code === ValidationErrorCode.INVALID_ACCOUNT_ID) {
      console.error("Account IDs must be canonical I105 literals only (no @domain suffix)", {
        field: error.path,
        cause: error.cause,
      });
    }
    throw error;
  }
  throw error;
}
```

## Bundle-size guardrails

Validation helpers are surfaced to end users, so additions should avoid large
dependencies and keep bundle impact visible. Run `npm run report:bundle-size`
after adding new validation helpers to refresh the bundle summary and attach it
to release evidence alongside tests.
