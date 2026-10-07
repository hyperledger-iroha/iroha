//! Declares the `iroha_plonk_oracle` cfg for `check-cfg`.
//!
//! Oracle mode (spec section 6.4: the injected vendored `transcript_repr`, the
//! `fe_to_fe` Poseidon point absorption and caller-seeded prover randomness)
//! is compiled only when `--cfg iroha_plonk_oracle` is passed through
//! `RUSTFLAGS` into a separate target directory. `ci/native_prover_oracle.py`
//! runs the independent oracle and requires the shipping Kaigi consumer to
//! fail its compile-time production guard. It is not a Cargo feature: resolver-2 feature
//! unification would otherwise compile the hooks into shipping binaries.
//! Because a stray `RUSTFLAGS` setting could still do so, shipping roots
//! assert `!iroha_plonk::ORACLE_BUILD` at compile time. This script only
//! declares the cfg name so the workspace `unexpected_cfgs` lint accepts it;
//! it reads no environment and generates no code.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(iroha_plonk_oracle)");
    println!("cargo::rerun-if-changed=build.rs");
}
