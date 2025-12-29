# soranet_pq

`soranet_pq` provides the post-quantum cryptography building blocks required by
the SoraNet handshake and relay tooling. It wraps the PQClean-backed ML-KEM
(FIPS 203) and ML-DSA (FIPS 204) parameter sets via the `pqcrypto` crates and
exposes helpers for deterministic HKDF derivations and hedged randomness.

## Features

- ML-KEM-512/768/1024 key generation, encapsulation, and decapsulation helpers.
- ML-DSA-44/65/87 key generation, detached signing, and verification helpers.
- HKDF derivation with protocol domain labels (`DH/es`, `KEM/1`, …).
- ChaCha20-based hedged RNG that mixes seed material with live OS entropy.

## Example

```rust
use soranet_pq::{
    encapsulate_mlkem, decapsulate_mlkem, generate_mlkem_keypair, MlKemSuite,
    generate_mldsa_keypair, sign_mldsa, verify_mldsa, MlDsaSuite,
};

// ML-KEM handshake
let kem_keys = generate_mlkem_keypair(MlKemSuite::MlKem768);
let (shared_a, ct) = encapsulate_mlkem(MlKemSuite::MlKem768, kem_keys.public_key()).unwrap();
let shared_b = decapsulate_mlkem(MlKemSuite::MlKem768, kem_keys.secret_key(), ct.as_bytes()).unwrap();
assert_eq!(shared_a.as_bytes(), shared_b.as_bytes());

// ML-DSA signatures
let dsa_keys = generate_mldsa_keypair(MlDsaSuite::MlDsa65)
    .expect("ML-DSA keypair generation should succeed");
let message = b"taikai-circuit";
let sig = sign_mldsa(MlDsaSuite::MlDsa65, dsa_keys.secret_key(), message).unwrap();
verify_mldsa(MlDsaSuite::MlDsa65, dsa_keys.public_key(), message, sig.as_bytes()).unwrap();
```

## Notes

- Secrets are wrapped in `Zeroizing` containers so memory is scrubbed on drop.
- The PQClean implementations already include the constant-time checks mandated
  by the standards; the crate adds domain separation helpers so the handshake
  can safely feed the outputs into HKDF transcript chaining.

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
