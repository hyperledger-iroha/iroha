# soranet_pq

`soranet_pq` provides the post-quantum cryptography building blocks required by
the SoraNet handshake and relay tooling. It wraps the current ML-KEM
(FIPS 203) and ML-DSA (FIPS 204) parameter sets and exposes helpers for
deterministic HKDF derivations and hedged randomness.

## Features

- ML-KEM-512/768/1024 key generation, encapsulation, and decapsulation helpers.
- ML-DSA-44/65/87 key generation, detached signing, and verification helpers.
- HKDF derivation with protocol domain labels (`DH/es`, `KEM/1`, …).
- ChaCha20-based hedged RNG that mixes seed material with live OS entropy and
  reports whether the OS entropy draw succeeded.

## Example

```rust
use soranet_pq::{
    decapsulate_mlkem, encapsulate_mlkem, generate_mldsa_keypair_from_os,
    generate_mlkem_keypair_from_os, sign_mldsa_from_os, verify_mldsa, MlDsaSuite,
    MlKemSuite,
};

// ML-KEM handshake
let kem_keys = generate_mlkem_keypair_from_os(MlKemSuite::MlKem768).unwrap();
let (shared_a, ct) = soranet_pq::encapsulate_mlkem_from_os(
    MlKemSuite::MlKem768,
    kem_keys.public_key(),
)
.unwrap();
let shared_b = decapsulate_mlkem(MlKemSuite::MlKem768, kem_keys.secret_key(), ct.as_bytes()).unwrap();
assert_eq!(shared_a.as_bytes(), shared_b.as_bytes());

// ML-DSA signatures
let dsa_keys = generate_mldsa_keypair_from_os(MlDsaSuite::MlDsa65)
    .expect("ML-DSA keypair generation should succeed");
let message = b"taikai-circuit";
let sig = sign_mldsa_from_os(MlDsaSuite::MlDsa65, dsa_keys.secret_key(), b"", message).unwrap();
verify_mldsa(MlDsaSuite::MlDsa65, dsa_keys.public_key(), b"", message, sig.as_bytes()).unwrap();
```

## Notes

- Secrets are wrapped in `Zeroizing` containers so memory is scrubbed on drop.
- ML-KEM validation rejects noncanonical 12-bit polynomial coefficients
  (values outside `q = 3329`) in public keys and in the secret-key private
  component, and secret-key validation applies the same check to the embedded
  public key before verifying `H(ek)`.
- ML-DSA secret-key validation reconstructs the committed public key, verifies
  the embedded `tr = H(pk)` value, and rejects noncanonical or internally
  inconsistent secret material before signing draws randomness.
- ML-KEM and ML-DSA entry points now require explicit hedged RNG objects or the
  fallible `_from_os` convenience helpers.
- The current backend uses the pqcrypto/PQClean FIPS implementations and calls
  the PQClean derandomized hooks directly where the Rust wrappers do not expose
  RNG injection. That keeps seeded and hedged randomness on the public execution
  path while the backend remains replaceable behind the same API.

## C FFI

`soranet_pq` emits the `cdylib` and `staticlib` artifacts whenever it is the
primary package being built (for example, `cargo build -p soranet_pq`). This
prevents workspace consumers from repeatedly generating the same FFI artifacts,
which in turn avoids the Cargo warning about colliding output filenames, while
keeping the developer workflow unchanged when explicitly building this crate.
A ready-to-use header lives at `crates/soranet_pq/include/soranet_pq.h` and
mirrors the exported symbols in `src/ffi.rs`:

```bash
# Build the release library and copy the header for your project
cargo build -p soranet_pq --release
cp crates/soranet_pq/include/soranet_pq.h /path/to/project/include/
```

Developers that have [`cbindgen`](https://github.com/mozilla/cbindgen) installed
can regenerate the header after editing the FFI surface:

```bash
cbindgen --config crates/soranet_pq/cbindgen.toml \
         --crate soranet_pq \
         --output crates/soranet_pq/include/soranet_pq.h
```

The generated header exposes the `soranet_mlkem_*` and `soranet_mldsa_*` entry
points alongside the shared error codes used throughout the C ABI. When you
need these artifacts during a workspace-wide build (where `soranet_pq` is just a
dependency), enable the `ffi-artifacts` feature explicitly:

```bash
# Build workspace benches/tests and still emit the FFI libraries
cargo bench --features soranet_pq/ffi-artifacts --no-run
```

You can also add `features = ["ffi-artifacts"]` to the dependency entry in
`Cargo.toml` if another crate always needs the artifacts.
