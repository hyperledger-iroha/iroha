# Iroha dependency patch

Upstream: ark-bls12-381 0.5.0, crates.io archive SHA-256
`3df4dcc01ff89867cd86b0da835f23c3f02738353aaee7dde7495af71363b8d5`.

The G1 subgroup check multiplies by the public curve parameter using Arkworks'
fixed-space double-and-add operation. It avoids the GLV scalar decomposition's
heap allocation while retaining both subgroup equations, the early rejection,
and checked canonical decoding. General scalar multiplication keeps upstream
GLV acceleration. Tests compare the check against multiplication by the subgroup
order for both subgroup and arbitrary on-curve points.

The crate always links Rust `std`; its retired no-std crate attribute is removed.
This does not change curve arithmetic or subgroup admission.

The normalized Cargo.toml is active; Cargo.toml.orig retains the upstream
manifest. Remove this patch when upstream provides a fixed-space subgroup check.
