# Iroha foundational model

`iroha_model_base` owns canonical `chain::ChainId`, `domain::DomainId`, `name::Name`,
`state_path::StatePath`, `metadata::{Metadata, Path}`, `error::ParseError` and
`topology::{DataSpaceId, LaneId, ShardId, LaneIdError}` and `peer::PeerId`.
Import these types directly
from their owning modules.
Ledger entities, instructions, queries, account registration and network/block
composition remain in `iroha_data_model`.

The types preserve their declared Norito identities, schema identifiers, exact
NFC spellings and bounded binary/JSON decoding. `Name::normalize` is the explicit
normalization operation; parsing requires an already canonical spelling. The
shared ICU data fingerprint and byte limits are part of that validation.

Chain labels are exact, case-sensitive ASCII with a 128-byte bound. Their
structural codecs enforce the same grammar as their constructors; private codec
helpers remain private. The shared grammar and sole byte-limit definition live
in `iroha_primitives::chain_id`. Genesis-derived `NetworkId` remains in the
aggregate because it contains a block-header hash.

Domain identities contain exactly two DNS labels, `name.dataspace`. Constructors
validate and normalize the components; binary and JSON object-key decoders
require canonical wire spellings and preserve decode-allocation accounting.
Domain entities, registration and `IdBox` composition remain in the aggregate.

Topology identities own their numeric validation, JSON and binary codecs, schema
identities and applicable storage keys. Lane and dataspace catalogs, lifecycle
policy, ledger traits and `IdBox` composition remain in the aggregate.

Peer identity owns the public-key wrapper, canonical binary/JSON decoding and
JSON object keys. Supported cryptographic algorithms remain owned by
`iroha_crypto`; the base requests its JSON surface and forwards FFI exports.
The ledger `Peer` entity and its address/identity composition remain in the aggregate.

Metadata retains its canonical sequence-of-tuples binary layout, duplicate-key
rejection, decode-allocation accounting and streaming serializer. Its owner tests
include a separate allocation-tracking executable.

JSON object-key and `mv` storage-key implementations live with the owned types.
The `ffi_export` and `transparent_api` features forward the existing opaque FFI
and model-macro behavior. Metadata's shared opaque-handle operations use the
`iroha_model_base` symbol prefix; the aggregate composition point owns the single
global FFI deallocator. The crate has no aggregate-model dependency.

Owner tests enable the cryptographic algorithm variants present in the captured
schema. Check the minimal production dependency separately with
`cargo check -p iroha_model_base --lib --no-default-features`.

Run the owner tests with `cargo test -p iroha_model_base`. Mixed ledger framing,
schema and storage-key fixtures remain in the aggregate's `base_wire_fixtures`
tests and must also pass when these owners or codecs change. The original
private ChainId frame assertions run inside the base owner against the same
immutable shared capture.
