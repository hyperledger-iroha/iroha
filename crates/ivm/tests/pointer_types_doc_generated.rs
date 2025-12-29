// Verify the generated section in docs/pointer_abi.md matches the code.

#[test]
fn generated_pointer_types_section_is_up_to_date() {
    const BEGIN: &str = "<!-- BEGIN GENERATED POINTER TYPES -->";
    const END: &str = "<!-- END GENERATED POINTER TYPES -->";
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/pointer_abi.md");
    let text = std::fs::read_to_string(path).expect("read pointer_abi.md");
    let beg = text.find(BEGIN).expect("begin marker present");
    let end = text.find(END).expect("end marker present");
    let section = &text[beg..end + END.len()];
    let table = ivm::render_pointer_types_markdown_table();
    // Accept either presence or absence of trailing newline after END marker
    let expected = format!("{BEGIN}\n{table}{END}\n");
    assert_eq!(
        section.trim_end_matches('\n'),
        expected.trim_end_matches('\n'),
        "generated pointer type list out of date"
    );
}
