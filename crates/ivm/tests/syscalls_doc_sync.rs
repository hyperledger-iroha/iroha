//! Ensure `docs/syscalls.md` generated section exactly matches the runtime’s
//! enriched table (Number/Name/Args/Return/Gas). This fails on any drift.

#[test]
fn generated_syscalls_section_is_up_to_date() {
    const BEGIN: &str = "<!-- BEGIN GENERATED SYSCALLS -->";
    const END: &str = "<!-- END GENERATED SYSCALLS -->";
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/syscalls.md");
    let text = std::fs::read_to_string(&path).expect("read syscalls.md");
    let beg = text
        .find(BEGIN)
        .unwrap_or_else(|| panic!("begin marker not found in {}", path.display()));
    let end = text
        .find(END)
        .unwrap_or_else(|| panic!("end marker not found in {}", path.display()));
    let section = &text[beg..end + END.len()];
    let table = ivm::syscalls::render_syscalls_markdown_table();
    let expected = format!("{BEGIN}\n{table}{END}");
    assert_eq!(
        section, expected,
        "docs/syscalls.md out of date; run: cargo run -p ivm --bin gen_syscalls_doc -- --write"
    );
}
