//! Dedicated process entrypoint for the externally memory-guarded Kagemusha real-proof check.

fn main() {
    iroha_core::zk::kagemusha_v1_recursion::run_guarded_real_mint_authority_proof_v1();
}
