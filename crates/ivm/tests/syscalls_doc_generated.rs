// Verify the generated section in docs/syscalls.md matches the code.

#[test]
fn generated_syscall_list_section_is_up_to_date() {
    const BEGIN: &str = "<!-- BEGIN GENERATED SYSCALLS -->";
    const END: &str = "<!-- END GENERATED SYSCALLS -->";
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");
    let text = std::fs::read_to_string(path).expect("read syscalls.md");
    let beg = text.find(BEGIN).expect("begin marker present");
    let end = text.find(END).expect("end marker present");
    let section = &text[beg..end + END.len()];
    let table = ivm::syscalls::render_syscalls_markdown_table();
    // Be tolerant to trailing-newline differences around the END marker
    let expected = format!("{BEGIN}\n{table}{END}\n");
    assert_eq!(
        section.trim_end_matches('\n'),
        expected.trim_end_matches('\n'),
        "generated syscall list out of date"
    );
}
