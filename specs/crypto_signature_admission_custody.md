# Signature admission rejection custody

`iroha_crypto::verify_signature_borrowed` is the single uncached admission
dispatch. It borrows the retained compact key/signature and returns an unformatted
`SignatureVerificationError`. The ordinary `verify_signature_for_admission`
adapter calls that relation and materializes its existing `Error` diagnostic only
at the completed boundary. No alternative signature relation, wire format,
algorithm policy, success cache, or decoder is introduced.

`PublicKey::borrowed_algorithm` and `borrowed_parts` retain a fixed envelope
rejection before formatting a public error. The existing compact and public
adapters use the same envelope predicates. A known tag with an empty payload is
still an envelope; it must fail the algorithm's own canonical parser, preserving
its original diagnostic and error order.

Ed25519, secp256k1, both BLS orientations, all five GOST parameter sets, SM2 and
ML-DSA reuse their existing parsers and signature relations. BLS retains the
actual typed canonical parser error. SM2 retains the original distinguishing
identity. The existing ML-DSA admission convention maps its fixed key/relation
failure to `BadSignature`, including a build without the PQC implementation.
Only ordinary adapters render error strings. Ordinary `Signature::verify` caches
remain separate owners and have not acquired an execution-pool reservation.

The source tests cover malformed envelopes and public diagnostic precedence,
valid and rejected signatures for every enabled algorithm on fresh threads, and
fresh-process controls for valid, all-zero key, truncated key, and all-zero
signature inputs. The complete shipping feature set requires eleven algorithms
and forty-four actual child invocations. Each child selects one exact test, emits
an algorithm/case-bound Rust allocation receipt, and must report exactly one
passed test with zero failures or ignored tests. These controls use the same
single crate test allocator as the existing BLS controls. They do not measure
foreign allocator calls, stack consumption, or establish hardware qualification.

Musubi's controller/quorum validation, signing-hash codec failures and ordinary
signature calls have not migrated in this prerequisite. The existing model
controller predicate must remain the sole implementation when integrating the
fixed owner. Canonical streaming hash failures and any remaining allocations must
retain their original owner or typed original-pool refusal before semantic State
capture is admitted. No semantic materializer is activated by this interface.
