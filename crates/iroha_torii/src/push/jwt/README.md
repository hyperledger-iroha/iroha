# Push token signing

The private module implements the provider-specific JWT envelope with Norito
JSON and unpadded base64url. FCM always signs RS256 (RSA PKCS#1 v1.5/SHA-256);
APNs always signs ES256 (P-256/SHA-256 with a fixed 64-byte `r || s` signature).
The AWS-LC implementation previously used through `jsonwebtoken` still performs
the cryptographic operations. There is no JWT decoder or configurable algorithm.

FCM accepts RSA keys in PKCS#1 `RSA PRIVATE KEY` or PKCS#8 `PRIVATE KEY` PEM.
APNs accepts P-256 PKCS#8 `PRIVATE KEY` PEM; SEC1, other curves, public keys and
other key families are rejected. The caller bounds the complete FCM credential
JSON to 64 KiB and the APNs PEM file to 16 KiB. The internal PEM decoder also
rejects inputs above 64 KiB before decoding.

Each credential contains exactly one complete PEM section. CRLF and surrounding
ASCII whitespace are supported; preambles, nested or additional sections, and
trailing non-whitespace are rejected. `rustls-pki-types` performs PEM decoding.
The existing `der` implementation, reexported by P-256, checks that the decoded
input is one complete DER sequence, with no trailing bytes. AWS-LC then validates
the actual key structure, family, parameters and key consistency. These checks
do not interpret ASN.1 locally. The decoded DER buffer is zeroized on drop;
this does not claim erasure of the caller's PEM/JSON buffers or parser scratch.

Run the focused tests and opt-in benchmark from the workspace root:

```sh
cargo iroha-fast -- test --locked -p iroha_torii --lib push::
cargo iroha-fast -- test --locked -p iroha_torii --lib push::jwt::tests::benchmark_provider_tokens -- --ignored --nocapture
```

Tests preserve the token header/payload assertions, compare RS256 tokens from
both private-key formats against an OpenSSL known answer, and independently
verify APNs signatures with RustCrypto P-256. Negative cases cover key types,
malformed/ambiguous PEM, trailing DER, invalid keys and the input-size boundary.
The push-module filter also exercises the provider callers, credential-file
bounds, redirects, persistence and retry behavior. The fixture keys are disposable
and public; see [fixtures](fixtures/README.md).

The benchmark reports all nine samples and their median for key-load-and-sign
and complete PEM-to-token construction. Each sample contains 32 operations;
fixture construction is outside the timer. It does not isolate signing from key
validation. Measure on a quiet host before drawing latency conclusions.

For a historical comparison, compile the old `jsonwebtoken` source only into an
external standalone harness, never a workspace manifest or dev-dependency. Use
the same AWS-LC, Norito and key fixtures on both sides, verify RSA token equality
and both algorithms' signatures before timing, alternate measurement order, and
retain all samples. Compare complete token construction with complete token
construction, including PEM parsing and key validation on both sides.

## Results

On 2026-11-04, an external `rustc -O` harness compared `jsonwebtoken` 11.1.0
against this module on `aarch64-apple-darwin`. Both paths shared the exact cached
AWS-LC, Norito and base64 artifacts. Cached dependencies retained their original
Cargo optimization profiles, so this was not an end-to-end release build.

The table shows historical → local results. Times are process CPU medians from
nine alternating paired batches: 100 operations per side for RSA-2048 and 2,000
for P-256. FCM used its five claims; APNs used its two claims. A separate binary
counted Rust allocator calls and requested bytes over 50 warmed operations.

| Operation / key | CPU µs/op | Allocations/op | Requested bytes/op |
| --- | ---: | ---: | ---: |
| Parse/sign / RSA PKCS#1 | 2,339.26 → 2,262.65 | 42 → 9 | 13,499 → 5,646 |
| Complete token / RSA PKCS#1 | 2,695.08 → 2,694.35 | 68 → 31 | 17,908 → 9,186 |
| Parse/sign / RSA PKCS#8 | 3,048.66 → 2,980.67 | 38 → 9 | 12,634 → 5,673 |
| Complete token / RSA PKCS#8 | 2,806.95 → 2,783.67 | 64 → 31 | 17,043 → 9,213 |
| Parse/sign / P-256 PKCS#8 | 62.12 → 55.56 | 36 → 8 | 3,928 → 1,508 |
| Complete token / P-256 PKCS#8 | 63.74 → 55.27 | 62 → 30 | 6,381 → 3,664 |

Complete RSA construction used essentially unchanged CPU time; APNs construction
used about 13% less in this measurement. Concurrent Cargo/rustc work dominated
wall time, preventing a reliable latency comparison. Allocation counts exclude
native C allocations inside AWS-LC; requested bytes are cumulative, not peak
memory. Allocation instrumentation was absent from the timing binary.

The harness checked identical unsigned token sections, exact RSA token equality,
and both algorithms' signatures through AWS-LC and independent OpenSSL
verification. Raw samples, source snapshots, compiler commands and result
provenance remain outside the repository in `/tmp/iroha-push-jwt-comparison`;
the historical dependency was not restored to any manifest.
