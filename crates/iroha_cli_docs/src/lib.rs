//! Markdown documentation for the declared command surfaces of Iroha native CLIs.
//!
//! The renderer consumes a command tree. It records declared argument identities before
//! building each command from the root downward, so inherited visibility policies take effect
//! without adding generated or inherited arguments to the documentation. Command subtrees are
//! never cloned. Hidden commands, arguments, defaults and possible values are omitted from
//! generated content; explicitly authored help prose and custom usage are preserved.

use std::{fmt::Write as _, io};

use clap::{Arg, ArgAction, Command, Id, builder::StyledStr};

/// Render the declared command tree as Markdown.
///
/// Help descriptions retain their author-provided Markdown. Names, aliases and values are
/// quoted as code; names used in links are reduced to heading anchors. The command is consumed
/// because Clap builds its usage in place.
///
/// For commands with hidden arguments, inferred usage shows visible required declarations and
/// groups without expanding cross-argument requirements. This prevents a visible argument's
/// dependency from revealing a hidden argument. Explicitly authored usage and help are retained.
#[must_use]
pub fn render_markdown(command: Command) -> String {
    let document = Document::prepare(command);
    let mut output = String::with_capacity(document.output_len());
    document
        .for_each_part(|part| {
            output.push_str(part);
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap_or_else(|never| match never {});
    output
}

/// Write Markdown to an output stream without allocating a second complete document.
///
/// # Errors
/// Returns the output stream's first write error.
pub fn write_markdown(command: Command, output: &mut impl io::Write) -> io::Result<()> {
    Document::prepare(command).for_each_part(|part| output.write_all(part.as_bytes()))
}

struct Document {
    text: String,
    usage_offsets: Vec<usize>,
    usages: Vec<String>,
}

struct DeclaredCommand {
    name: String,
    source_name: String,
    arguments: Vec<(Id, bool)>,
    children: Vec<Self>,
}

impl DeclaredCommand {
    fn capture(command: &Command) -> Self {
        Self {
            name: canonical_name(command).to_owned(),
            source_name: command.get_name().to_owned(),
            arguments: command
                .get_arguments()
                .map(|arg| (arg.get_id().clone(), !arg.get_default_values().is_empty()))
                .collect(),
            children: command.get_subcommands().map(Self::capture).collect(),
        }
    }

    fn arguments<'a>(&'a self, command: &'a Command) -> impl Iterator<Item = (&'a Arg, bool)> {
        self.arguments.iter().filter_map(move |(id, has_default)| {
            command
                .get_arguments()
                .find(|arg| arg.get_id() == id)
                .filter(|arg| !arg.is_hide_set())
                .map(|arg| (arg, *has_default))
        })
    }
}

impl Document {
    fn prepare(mut command: Command) -> Self {
        let mut document = Self {
            text: String::new(),
            usage_offsets: Vec::new(),
            usages: Vec::new(),
        };
        if command.is_hide_set() {
            return document;
        }
        let declared = DeclaredCommand::capture(&command);
        document.text.push_str("# Command-Line Help for ");
        code(&mut document.text, canonical_name(&command));
        document
            .text
            .push_str("\n\nThis document contains the help content for the ");
        code(&mut document.text, canonical_name(&command));
        document
            .text
            .push_str(" command-line program.\n\n**Command Overview:**\n\n");
        let mut path = Vec::new();
        table_of_contents(&command, &mut path, &mut document.text);
        document.text.push('\n');
        collect_usages(
            &mut command,
            &declared,
            &mut Vec::new(),
            &mut document.usages,
        );
        sections(&command, &declared, &mut Vec::new(), &mut document);
        document
            .text
            .truncate(document.text.trim_end_matches('\n').len());
        document.text.push('\n');
        document
    }

    fn output_len(&self) -> usize {
        self.text.len() + self.usages.iter().map(String::len).sum::<usize>()
    }

    fn for_each_part<E>(&self, mut write: impl FnMut(&str) -> Result<(), E>) -> Result<(), E> {
        let mut start = 0;
        for (&offset, usage) in self.usage_offsets.iter().zip(&self.usages) {
            write(&self.text[start..offset])?;
            write(usage)?;
            start = offset;
        }
        write(&self.text[start..])
    }
}

fn canonical_name(command: &Command) -> &str {
    command
        .get_display_name()
        .or_else(|| command.get_bin_name())
        .unwrap_or_else(|| command.get_name())
}

fn table_of_contents<'a>(command: &'a Command, path: &mut Vec<&'a str>, out: &mut String) {
    if command.is_hide_set() {
        return;
    }
    path.push(canonical_name(command));
    out.push_str("* [");
    path_code(out, path);
    out.push_str("↴](#");
    for (index, name) in path.iter().enumerate() {
        if index != 0 {
            out.push('-');
        }
        for ch in name.chars().flat_map(char::to_lowercase) {
            if ch.is_alphanumeric() || matches!(ch, '-' | '_') {
                out.push(ch);
            } else if ch.is_whitespace() {
                out.push('-');
            }
        }
    }
    out.push_str(")\n");
    for child in command.get_subcommands() {
        table_of_contents(child, path, out);
    }
    path.pop();
}

fn sections<'a>(
    command: &Command,
    declared: &'a DeclaredCommand,
    path: &mut Vec<&'a str>,
    document: &mut Document,
) {
    if command.is_hide_set() {
        return;
    }
    path.push(&declared.name);
    let out = &mut document.text;
    out.push_str("## ");
    path_code(out, path);
    out.push_str("\n\n");
    paragraph(
        out,
        command.get_long_about().or_else(|| command.get_about()),
    );
    paragraph(
        out,
        command
            .get_before_long_help()
            .or_else(|| command.get_before_help()),
    );
    // The complete usage is quoted later: custom usage may itself contain backticks.
    out.push_str("**Usage:** ");
    document.usage_offsets.push(out.len());
    out.push_str("\n\n");
    let mut aliases = command.get_visible_aliases().peekable();
    if aliases.peek().is_some() {
        let count = command.get_visible_aliases().count();
        out.push_str(if count == 1 {
            "**Command Alias:** "
        } else {
            "**Command Aliases:** "
        });
        code_list(out, aliases);
        out.push_str("\n\n");
    }
    paragraph(
        out,
        command
            .get_after_long_help()
            .or_else(|| command.get_after_help()),
    );
    let children = || {
        declared.children.iter().filter_map(|child| {
            command
                .find_subcommand(&child.source_name)
                .filter(|command| !command.is_hide_set())
                .map(|command| (command, child))
        })
    };
    if children().next().is_some() {
        out.push_str("###### **Subcommands:**\n\n");
        for (child, source) in children() {
            out.push_str("* ");
            code(out, &source.name);
            out.push_str(" — ");
            if let Some(about) = child.get_about() {
                write!(out, "{about}").unwrap();
            }
            out.push('\n');
        }
        out.push('\n');
    }
    if declared
        .arguments(command)
        .any(|(arg, _)| arg.is_positional())
    {
        out.push_str("###### **Arguments:**\n\n");
        for (arg, has_default) in declared
            .arguments(command)
            .filter(|(arg, _)| arg.is_positional())
        {
            argument(out, arg, has_default);
        }
        out.push('\n');
    }
    let options = || {
        declared
            .arguments(command)
            .filter(|(arg, _)| !arg.is_positional())
    };
    if options().next().is_some() {
        out.push_str("###### **Options:**\n\n");
        for (arg, has_default) in options() {
            argument(out, arg, has_default);
        }
        out.push('\n');
    }
    out.push_str("\n\n");
    for (child, source) in children() {
        sections(child, source, path, document);
    }
    path.pop();
}

fn collect_usages<'a>(
    command: &mut Command,
    declared: &'a DeclaredCommand,
    path: &mut Vec<&'a str>,
    usages: &mut Vec<String>,
) {
    if command.is_hide_set() {
        return;
    }
    // Markdown already gives every command its own section. Flattening here would make
    // Clap clone and inline children before their hidden arguments have been projected out.
    *command = std::mem::take(command).flatten_help(false);
    // Building from the root down applies command-wide and inherited visibility policy.
    let usage = command.render_usage().to_string();
    let usage = if command.get_overridden_usage().is_none()
        && command.get_arguments().any(Arg::is_hide_set)
    {
        visible_usage(command)
    } else {
        usage
    };
    let mut full_usage = path.join(" ");
    if !full_usage.is_empty() {
        full_usage.push(' ');
    }
    full_usage.push_str(usage.strip_prefix("Usage: ").unwrap_or(&usage));
    let mut quoted = String::new();
    code(&mut quoted, &full_usage);
    usages.push(quoted);
    path.push(&declared.name);
    for source in &declared.children {
        if let Some(child) = command.find_subcommand_mut(&source.source_name) {
            collect_usages(child, source, path, usages);
        }
    }
    path.pop();
}

fn visible_usage(command: &mut Command) -> String {
    let hidden: Vec<Id> = command
        .get_arguments()
        .filter(|arg| arg.is_hide_set())
        .map(|arg| arg.get_id().clone())
        .collect();
    let groups: Vec<_> = command
        .get_groups()
        .map(|group| {
            let members: Vec<_> = group
                .get_args()
                .filter(|id| !hidden.contains(id))
                .cloned()
                .collect();
            let required = group.is_required_set() && !members.is_empty();
            (group.get_id().clone(), members, required)
        })
        .collect();
    // This is a usage-only projection of an already built command. Keep all IDs present so
    // conditional references remain valid, but never expand `requires` into hidden names.
    // Parsing uses the caller's original factory and is unaffected by the consumed document tree.
    let mut projection = std::mem::take(command).mut_args(|arg| {
        let arg = arg.requires(clap::builder::Resettable::Reset);
        if arg.is_hide_set() {
            arg.required(false).group(clap::builder::Resettable::Reset)
        } else {
            arg
        }
    });
    for (id, members, required) in groups {
        projection = projection.mut_group(id, |group| {
            group
                .arg(clap::builder::Resettable::Reset)
                .args(members)
                .required(required)
                .requires(clap::builder::Resettable::Reset)
        });
    }
    *command = projection;
    command.render_usage().to_string()
}

fn paragraph(out: &mut String, text: Option<&StyledStr>) {
    if let Some(text) = text {
        writeln!(out, "{text}\n").unwrap();
    }
}

fn argument(out: &mut String, arg: &Arg, has_declared_default: bool) {
    out.push_str("* ");
    let mut spelling = String::new();
    if let Some(short) = arg.get_short() {
        write!(spelling, "-{short}").unwrap();
        if arg.get_long().is_some() {
            code(out, &spelling);
            out.push_str(", ");
            spelling.clear();
        }
    }
    if let Some(long) = arg.get_long() {
        write!(spelling, "--{long}").unwrap();
    }
    if arg.is_positional() || arg.get_action().takes_values() {
        if !spelling.is_empty() {
            spelling.push(' ');
        }
        if let Some(names) = arg.get_value_names().filter(|names| !names.is_empty()) {
            for (index, name) in names.iter().enumerate() {
                if index != 0 {
                    spelling.push(' ');
                }
                write!(spelling, "<{name}>").unwrap();
            }
        } else {
            write!(spelling, "<{}>", arg.get_id().as_str().to_ascii_uppercase()).unwrap();
        }
    }
    code(out, &spelling);
    if let Some(aliases) = arg
        .get_visible_aliases()
        .filter(|aliases| !aliases.is_empty())
    {
        out.push_str(if aliases.len() == 1 {
            " [alias: "
        } else {
            " [aliases: "
        });
        code_list(out, aliases.into_iter());
        out.push(']');
    }
    if let Some(help) = arg.get_long_help() {
        indented_help(out, &help.to_string());
    } else if let Some(help) = arg.get_help() {
        writeln!(out, " — {help}").unwrap();
    } else {
        out.push('\n');
    }
    let defaults = arg.get_default_values();
    if has_declared_default && !arg.is_hide_default_value_set() && !defaults.is_empty() {
        out.push_str(if defaults.len() == 1 {
            "\n  Default value: "
        } else {
            "\n  Default values: "
        });
        for (index, value) in defaults.iter().enumerate() {
            if index != 0 {
                out.push_str(", ");
            }
            code(out, &value.to_string_lossy());
        }
        out.push('\n');
    }
    if arg.is_hide_possible_values_set()
        || matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::SetFalse)
    {
        return;
    }
    let values = arg.get_possible_values();
    let visible = || values.iter().filter(|value| !value.is_hide_set());
    if visible().next().is_none() {
        return;
    }
    if visible().any(|value| value.get_help().is_some()) {
        out.push_str("\n  Possible values:\n");
        for value in visible() {
            out.push_str("  - ");
            code(out, value.get_name());
            if let Some(help) = value.get_help() {
                write!(out, ":\n    {help}").unwrap();
            }
            out.push('\n');
        }
        out.push('\n');
    } else {
        out.push_str("\n  Possible values: ");
        code_list(out, visible().map(|value| value.get_name()));
        out.push_str("\n\n");
    }
}

fn indented_help(out: &mut String, help: &str) {
    let mut first = true;
    if help.is_empty() {
        out.push('\n');
    }
    for line in help.lines() {
        if !line.is_empty() {
            out.push_str(if first { " — " } else { "   " });
            out.push_str(line);
            first = false;
        }
        out.push('\n');
    }
}

fn code_list<'a>(out: &mut String, values: impl Iterator<Item = &'a str>) {
    for (index, value) in values.enumerate() {
        if index != 0 {
            out.push_str(", ");
        }
        code(out, value);
    }
}

fn path_code(out: &mut String, path: &[&str]) {
    code_parts(out, path.iter().copied());
}

fn code(out: &mut String, text: &str) {
    code_parts(out, std::iter::once(text));
}

fn code_parts<'a>(out: &mut String, parts: impl Iterator<Item = &'a str> + Clone) {
    // A longer delimiter prevents source-defined names/defaults from breaking out of code.
    // Newlines cannot become Markdown structure inside a code span.
    let fence = parts
        .clone()
        .flat_map(|part| part.split(|ch| ch != '`'))
        .map(str::len)
        .max()
        .unwrap_or(0)
        + 1;
    let padding = parts
        .clone()
        .next()
        .is_some_and(|part| part.starts_with(['`', ' ']))
        || parts
            .clone()
            .last()
            .is_some_and(|part| part.ends_with(['`', ' ']));
    out.extend(std::iter::repeat_n('`', fence));
    if padding {
        out.push(' ');
    }
    for (index, part) in parts.enumerate() {
        if index != 0 {
            out.push(' ');
        }
        if part.contains(['\r', '\n']) {
            out.extend(
                part.chars()
                    .map(|ch| if matches!(ch, '\r' | '\n') { ' ' } else { ch }),
            );
        } else {
            out.push_str(part);
        }
    }
    if padding {
        out.push(' ');
    }
    out.extend(std::iter::repeat_n('`', fence));
}

#[cfg(test)]
mod tests;
