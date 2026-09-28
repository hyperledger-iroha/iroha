# Contextual BLS single-signature verification

Both BLS orientations use `canonical.rs` for checked, canonical, nonidentity
point parsing and `uncached.rs` for the single-signature relation. The checked
W3f/Ark decoder writes roundtrips into fixed 96-byte backing. Conversion to the
fixed blstrs point representation also checks canonical encoding, subgroup and
identity. The synchronous blst core borrows the message without retaining it.

The transcript is Iroha's existing W3f basic-ciphersuite prefix followed by
`for signing messages` and the message, with the one-byte hash-to-field DST
`[1]`. It is not the Ethereum PoP transcript. Wire bytes and this transcript
are shared across all hardware.

`Signature::verify` validates the public key first, then returns `BadSignature`
for all-zero or wrong-length signature material. The typed `BlsImpl::verify`
diagnoses all-zero signature material before typed-key identity. Other parser
reasons retain exact canonical/identity/decoder distinctions. Supplied typed
keys must also pass subgroup validation; manually constructing a point does
not bypass the public verification contract.

Ordinary verification may retain an exact positive-verdict cache. The digest
only selects a bucket; full public-key, message and signature bytes establish
a hit, after canonical parsing. It does not retain a prepared-key cache or
enter the generic decoded-key cache for BLS. Admission calls the same parser
and relation without any retained verification cache.

The parser retains the actual typed `ark_serialize::SerializationError`; it
never infers a decoder reason from input flags. The supported borrowed point
decoders return allocation-free failures. The ordinary public `Error::Parse`
adapter materializes its exact diagnostic String outside that core.
Positive-cache backing, public diagnostic Strings, aggregate verification,
PoP cache retention and signing allocations have separate lifetime owners.
This single-signature boundary does not establish their funding or complete
authenticated-State capture.

`uncached_tests.rs` verifies independently generated W3f and blst signatures,
exact upstream message points and allocation observation. Permanent
`consolidation_tests.rs` controls assert a compact table of specified parser
diagnostics, facade precedence, every compressed-flag family, field boundaries,
suffixes, typed subgroup tripwires and retained-cache bypass. Shipping source
contains no copied retired parser or single-signature verifier.
