# Iroha foundational model

`iroha_model_base` owns canonical `name::Name`, `state_path::StatePath` and
`error::ParseError`. Import these types directly from their owning modules.
Ledger entities, instructions, queries, account registration and network/block
composition remain in `iroha_data_model`.

The types preserve their declared Norito identities, schema identifiers, exact
NFC spellings and bounded binary/JSON decoding. `Name::normalize` is the explicit
normalization operation; parsing requires an already canonical spelling. The
shared ICU data fingerprint and byte limits are part of that validation.

JSON object-key and `mv` storage-key implementations live with the owned types.
The `ffi_export` and `transparent_api` features forward the existing opaque FFI
and model-macro behavior. The crate has no aggregate-model dependency.

Run the owner tests with `cargo test -p iroha_model_base`. Mixed ledger framing,
schema and storage-key fixtures remain in the aggregate's `base_wire_fixtures`
tests and must also pass when these owners or codecs change.
