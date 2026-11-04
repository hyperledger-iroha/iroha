//! Coverage for documentation shape, visibility and safe Markdown quoting.

use super::*;
use clap::{ArgGroup, builder::PossibleValue};

#[test]
fn renders_declared_tree_in_order_without_generated_commands() {
    let command = Command::new("tool")
        .about("Short overview")
        .long_about("Long overview")
        .before_help("Before")
        .after_help("After")
        .subcommand(Command::new("first").about("First child"))
        .subcommand(Command::new("second").subcommand(Command::new("leaf")));
    let rendered = render_markdown(command);
    assert!(rendered.starts_with("# Command-Line Help for `tool`\n"));
    assert!(rendered.contains("* [`tool second leaf`↴](#tool-second-leaf)"));
    assert!(rendered.contains("## `tool`\n\nLong overview\n\nBefore\n\n**Usage:**"));
    assert!(rendered.contains("**Usage:** `tool second leaf`"));
    assert!(rendered.find("## `tool first`").unwrap() < rendered.find("## `tool second`").unwrap());
    assert!(!rendered.contains("## `tool help`"));
    assert!(!rendered.contains("clap-markdown"));
}

#[test]
fn inherited_arguments_do_not_duplicate_declared_documentation() {
    let command = Command::new("tool")
        .arg(
            Arg::new("shared")
                .long("shared")
                .global(true)
                .default_value("root-default"),
        )
        .subcommand(Command::new("leaf").arg(Arg::new("local").long("local")));
    let rendered = render_markdown(command);
    let child = rendered.split("## `tool leaf`\n").nth(1).unwrap();
    assert!(child.contains("**Usage:** `tool leaf [OPTIONS]`"));
    assert!(child.contains("`--local <LOCAL>`"));
    assert!(!child.contains("shared"));
    assert!(!child.contains("root-default"));
}

#[test]
fn hidden_required_arguments_never_appear_in_inferred_usage() {
    let command = Command::new("tool")
        .arg(Arg::new("hidden-position").required(true).hide(true))
        .arg(Arg::new("visible-position").required(true))
        .arg(
            Arg::new("hidden-option")
                .long("hidden-option")
                .required(true)
                .hide(true),
        )
        .arg(
            Arg::new("visible-option")
                .long("visible-option")
                .required(true)
                .requires("hidden-option"),
        )
        .arg(
            Arg::new("conditional")
                .long("conditional")
                .required_if_eq("hidden-option", "value"),
        );
    let rendered = render_markdown(command);
    assert!(!rendered.to_lowercase().contains("hidden-"));
    let usage = rendered
        .lines()
        .find(|line| line.starts_with("**Usage:**"))
        .unwrap();
    assert!(
        usage.contains("--visible-option <visible-option>"),
        "{usage}"
    );
    assert!(usage.contains("<visible-position>"));
}

#[test]
fn required_groups_exclude_hidden_members_and_empty_groups() {
    let command = Command::new("tool")
        .arg(
            Arg::new("visible")
                .long("visible")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("hidden-one")
                .long("hidden-one")
                .hide(true)
                .action(ArgAction::SetTrue)
                .group("mixed"),
        )
        .arg(
            Arg::new("hidden-two")
                .long("hidden-two")
                .hide(true)
                .action(ArgAction::SetTrue),
        )
        .group(
            ArgGroup::new("mixed")
                .args(["visible", "hidden-one"])
                .required(true),
        )
        .group(
            ArgGroup::new("all-hidden")
                .args(["hidden-two"])
                .required(true),
        );
    let rendered = render_markdown(command);
    assert!(!rendered.to_lowercase().contains("hidden"));
    let usage = rendered
        .lines()
        .find(|line| line.starts_with("**Usage:**"))
        .unwrap();
    assert!(usage.contains("<--visible>"));
    assert!(!usage.contains("<>"));
}

#[test]
fn command_wide_value_visibility_is_applied_to_children() {
    let command = Command::new("tool")
        .hide_possible_values(true)
        .arg(
            Arg::new("mode")
                .long("mode")
                .value_parser(["root-hidden-enum"]),
        )
        .subcommand(
            Command::new("child").arg(
                Arg::new("mode")
                    .long("mode")
                    .value_parser(["child-hidden-enum"]),
            ),
        );
    let rendered = render_markdown(command);
    assert!(!rendered.contains("root-hidden-enum"));
    assert!(!rendered.contains("child-hidden-enum"));
    assert!(!rendered.contains("Possible values:"));
    assert!(rendered.contains("`--mode <MODE>`"));
}

#[test]
fn hidden_root_has_no_generated_document() {
    let command = Command::new("hidden-root")
        .hide(true)
        .subcommand(Command::new("child"));
    assert!(render_markdown(command.clone()).is_empty());
    let mut bytes = Vec::new();
    write_markdown(command, &mut bytes).unwrap();
    assert!(bytes.is_empty());
}

#[test]
fn flattened_terminal_help_keeps_markdown_sections_and_hidden_arguments_separate() {
    let command = Command::new("tool").flatten_help(true).subcommand(
        Command::new("leaf")
            .flatten_help(true)
            .arg(Arg::new("secret").long("secret").hide(true).required(true))
            .arg(Arg::new("visible").long("visible").required(true))
            .subcommand(Command::new("nested")),
    );
    let rendered = render_markdown(command);
    assert!(!rendered.to_lowercase().contains("secret"));
    assert!(rendered.contains("## `tool leaf`\n"));
    assert!(rendered.contains("## `tool leaf nested`\n"));
    let usage = rendered
        .lines()
        .find(|line| line.starts_with("**Usage:**"))
        .unwrap();
    assert!(
        !usage.contains("--visible"),
        "child usage leaked into root: {usage}"
    );
    assert!(rendered.contains("--visible <visible>"));
}

#[test]
fn renders_argument_shapes_aliases_defaults_and_long_help() {
    let command = Command::new("tool")
        .visible_alias("alias")
        .arg(Arg::new("input").help("Input file"))
        .arg(Arg::new("switch").short('q').action(ArgAction::SetTrue))
        .arg(
            Arg::new("named")
                .long("named")
                .visible_alias("other")
                .default_value("one"),
        )
        .arg(
            Arg::new("pair")
                .short('p')
                .long("pair")
                .num_args(2)
                .value_names(["KEY", "VALUE"])
                .default_values(["first", "second"])
                .long_help("Heading\n\nSecond paragraph"),
        );
    let rendered = render_markdown(command);
    assert!(rendered.contains("**Command Alias:** `alias`"));
    assert!(rendered.contains("* `<INPUT>` — Input file"));
    assert!(rendered.contains("* `-q`\n"));
    assert!(rendered.contains("* `--named <NAMED>` [alias: `other`]"));
    assert!(rendered.contains("Default value: `one`"));
    assert!(rendered.contains("* `-p`, `--pair <KEY> <VALUE>` — Heading\n\n   Second paragraph\n"));
    assert!(rendered.contains("Default values: `first`, `second`"));
}

#[test]
fn suppresses_hidden_commands_arguments_defaults_and_values() {
    let command = Command::new("tool")
        .arg(Arg::new("private-position").hide(true))
        .arg(Arg::new("private-option").long("private-option").hide(true))
        .arg(
            Arg::new("token")
                .long("token")
                .default_value("secret-default")
                .hide_default_value(true),
        )
        .arg(Arg::new("mode").long("mode").value_parser([
            PossibleValue::new("public"),
            PossibleValue::new("private-value").hide(true),
        ]))
        .arg(
            Arg::new("hidden-values")
                .long("hidden-values")
                .value_parser(["private-enum"])
                .hide_possible_values(true),
        )
        .subcommand(
            Command::new("private-command")
                .hide(true)
                .subcommand(Command::new("private-child")),
        );
    let rendered = render_markdown(command);
    let lowercase = rendered.to_lowercase();
    for hidden in [
        "private-position",
        "private-option",
        "secret-default",
        "private-value",
        "private-enum",
        "private-command",
        "private-child",
    ] {
        assert!(!lowercase.contains(hidden), "leaked {hidden}");
    }
    assert!(!rendered.contains("###### **Arguments:**"));
    assert!(!rendered.contains("###### **Subcommands:**"));
    assert!(rendered.contains("Possible values: `public`"));
}

#[test]
fn possible_values_keep_help_and_filter_hidden_choices() {
    let command = Command::new("tool").arg(
        Arg::new("mode").long("mode").value_parser([
            PossibleValue::new("first").help("Choose first"),
            PossibleValue::new("second"),
            PossibleValue::new("hidden")
                .hide(true)
                .help("Do not reveal"),
        ]),
    );
    let rendered = render_markdown(command);
    assert!(
        rendered.contains("  Possible values:\n  - `first`:\n    Choose first\n  - `second`\n")
    );
    assert!(!rendered.contains("Do not reveal"));
}

#[test]
fn names_aliases_defaults_and_usage_cannot_break_code_spans() {
    let command = Command::new("tool")
        .display_name("Tool `name` [x](evil)")
        .visible_alias("`alias`")
        .override_usage("tool `literal`\n# not a heading")
        .arg(
            Arg::new("value")
                .long("value")
                .default_value("`<script>alert(1)</script>`"),
        );
    let rendered = render_markdown(command);
    assert!(rendered.contains("# Command-Line Help for ``Tool `name` [x](evil)``"));
    assert!(rendered.contains("](\u{23}tool-name-xevil)"));
    assert!(rendered.contains("**Command Alias:** `` `alias` ``"));
    assert!(rendered.contains("**Usage:** ``tool `literal` # not a heading``"));
    assert!(rendered.contains("Default value: `` `<script>alert(1)</script>` ``"));
    assert!(!rendered.contains("\n# not a heading"));
}

#[test]
fn canonical_names_follow_display_then_binary_then_command() {
    assert_eq!(canonical_name(&Command::new("name")), "name");
    assert_eq!(
        canonical_name(&Command::new("name").bin_name("binary")),
        "binary"
    );
    assert_eq!(
        canonical_name(
            &Command::new("name")
                .bin_name("binary")
                .display_name("display")
        ),
        "display"
    );
}

#[test]
fn code_quoting_and_multiline_help_cover_empty_and_whitespace() {
    for (input, expected) in [
        ("", "``"),
        ("a", "`a`"),
        (" ` ", "``  `  ``"),
        ("a``b", "```a``b```"),
        ("a\nb", "`a b`"),
    ] {
        let mut out = String::new();
        code(&mut out, input);
        assert_eq!(out, expected);
    }
    let mut out = String::new();
    indented_help(&mut out, "\nTitle\n\nSecond\n");
    assert_eq!(out, "\n — Title\n\n   Second\n");
    out.clear();
    indented_help(&mut out, "");
    assert_eq!(out, "\n");
}

#[test]
fn writer_matches_rendered_document_and_propagates_io_failure() {
    let command = Command::new("tool").subcommand(Command::new("leaf"));
    let expected = render_markdown(command.clone());
    assert!(expected.ends_with('\n'));
    assert!(!expected.ends_with("\n\n"));
    let mut bytes = Vec::new();
    write_markdown(command.clone(), &mut bytes).unwrap();
    assert_eq!(bytes, expected.as_bytes());
    struct Refuse;
    impl io::Write for Refuse {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        write_markdown(command, &mut Refuse).unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}
