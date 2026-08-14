//! Parameter default sanity checks.
use iroha_data_model::parameter::Parameters;
#[test]
fn sumeragi_defaults_have_nonzero_cadence() {
    let params = Parameters::default();
    assert_eq!(params.sumeragi.block_cadence_ms.get(), 100);
}
