//! Declares the `iroha_plonk_oracle` cfg for `check-cfg`.
//!
//! Oracle mode (spec section 6.4: the injected vendored `transcript_repr` and
//! the `fe_to_fe` Poseidon point absorption) is compiled only when the oracle
//! CI job passes `--cfg iroha_plonk_oracle` through `RUSTFLAGS` into its own
//! target directory. It is deliberately not a Cargo feature: resolver-2 feature
//! unification would otherwise compile the hooks into shipping binaries. This
//! script only declares the cfg name so the workspace `unexpected_cfgs` lint
//! accepts it; it reads no environment and generates no code.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(iroha_plonk_oracle)");
    println!("cargo::rerun-if-changed=build.rs");
}
