//! Node-local files and environment cannot select lifecycle protocol activation.

use super::*;
use iroha_config_base::{env::MockEnv, toml::TomlSource};

#[test]
fn lifecycle_protocol_switches_are_not_node_configuration() {
    for source in [
        "[lifecycle_trace]\nv2_activation_height = 42",
        "[lifecycle_trace]\nenabled = true",
        "[lifecycle_trace]\nactivation_height = 42",
    ] {
        let result = ConfigReader::new()
            .without_env()
            .with_toml_source(TomlSource::inline(toml::from_str(source).unwrap()))
            .read_and_complete::<Root>();
        let error = result.unwrap_err();
        let text = format!("{error:?}");
        assert!(text.contains("lifecycle_trace"), "{text}");
    }
}

#[test]
fn lifecycle_environment_has_no_activation_binding() {
    let environment = MockEnv::default()
        .set("LIFECYCLE_TRACE_V2_ACTIVATION_HEIGHT", "42")
        .set("IROHA_LIFECYCLE_TRACE_V2_ACTIVATION_HEIGHT", "42")
        .set("V2_ACTIVATION_HEIGHT", "42");
    // The process lifecycle remains an independent local resource setting.
    let lifecycle = ConfigReader::new()
        .with_env(environment.clone())
        .read_and_complete::<Lifecycle>()
        .unwrap();
    assert!(!lifecycle.exit_on_stdin_close);
    assert_eq!(environment.unvisited().len(), 3);
    assert!(environment.unknown().is_empty());
}
