//! Rejection test for the removed in-memory `Map` surface.
#[test]
fn in_memory_map_contains_is_rejected() {
    let src = r#"
        module RemovedMapContains {
            fn f() -> int {
                let m = Map::new();
                m[7] = 111;
                let present = m.contains(7);
                return present;
            }
        }
    "#;
    let error = ivm::KotodamaCompiler::new()
        .compile_source(src)
        .expect_err("V1 must reject the removed in-memory Map type");
    assert!(error.contains("Map"), "unexpected diagnostic: {error}");
}
