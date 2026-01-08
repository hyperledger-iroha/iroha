//! Parameter default sanity checks.

use iroha_data_model::parameter::Parameters;

#[test]
fn sumeragi_defaults_enable_da() {
    let params = Parameters::default();

    assert!(
        params.sumeragi.da_enabled,
        "sumeragi.da_enabled must default to true"
    );
}
