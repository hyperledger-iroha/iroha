# IVM ISO 20022 fixed assets

`iso20022_iban_specs_v1.bin` is the alphabetically ordered June 2024 IBAN
registry projection used by ISO 20022 validation. Each three-byte record is a
two-byte ASCII country code followed by its exact IBAN length. A fixed-size
`const fn` reconstructs the same `[IbanSpec; 78]`, exposed to the existing
consumer as the same `&[IbanSpec]`; binary-search order and runtime cost remain
unchanged.

The existing `msg_validate_rejects_invalid_iban`,
`msg_validate_accepts_valid_iban`, `camt053_rejects_invalid_iban`, and
`camt053_accepts_valid_message` tests retain the behavioral coverage.
`manifest.json` pins the asset and its pre-extraction Rust declaration.
