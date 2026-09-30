//! Iroha 3 daemon binary.
fn main() {
    irohad::main_entry(iroha_core::compiled_build_metadata!());
}
