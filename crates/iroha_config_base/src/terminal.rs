//! Terminal color detection shared by the node and CLI binaries.
//!
//! The rules follow the common environment conventions: `FORCE_COLOR` and
//! `CLICOLOR_FORCE` force colors on; `NO_COLOR`, `TERM=dumb` and streams that
//! are not terminals (unless `IGNORE_IS_TERMINAL` is set) turn them off;
//! otherwise `COLORTERM`, a color-capable `TERM`, a known `TERM_PROGRAM`,
//! `CLICOLOR` or a CI environment turn them on.
use std::{env, io::IsTerminal};

/// Standard stream whose color support is queried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminalStream {
    /// The process standard output.
    Stdout,
    /// The process standard error.
    Stderr,
}

/// Environment variables whose presence marks a CI environment.
const CI_MARKERS: &[&str] = &[
    "CI_NAME",
    "GITHUB_ACTION",
    "GITLAB_CI",
    "NETLIFY",
    "TRAVIS",
    "CODEBUILD_SRC_DIR",
    "BUILDER_OUTPUT",
    "GITLAB_DEPLOYMENT",
    "NOW_GITHUB_DEPLOYMENT",
    "NOW_BUILDER",
    "BITBUCKET_DEPLOYMENT",
    "GERRIT_PROJECT",
    "SYSTEM_TEAMFOUNDATIONCOLLECTIONURI",
    "BITRISE_IO",
    "BUDDY_WORKSPACE_ID",
    "BUILDKITE",
    "CIRRUS_CI",
    "APPVEYOR",
    "CIRCLECI",
    "SEMAPHORE",
    "DRONE",
    "DSARI",
    "TDDIUM",
    "STRIDER",
    "TASKCLUSTER_ROOT_URL",
    "JENKINS_URL",
    "bamboo.buildKey",
    "GO_PIPELINE_NAME",
    "HUDSON_URL",
    "WERCKER",
    "MAGNUM",
    "NEVERCODE",
    "RENDER",
    "SAIL_CI",
    "SHIPPABLE",
];

/// Whether ANSI colors should be written to `stream` in this process.
pub fn supports_color(stream: TerminalStream) -> bool {
    let is_terminal = match stream {
        TerminalStream::Stdout => std::io::stdout().is_terminal(),
        TerminalStream::Stderr => std::io::stderr().is_terminal(),
    };
    supports_color_with(is_terminal, |name| env::var(name).ok())
}

/// Color decision for a stream with the given terminal status and environment.
///
/// `var` returns a variable's value, or `None` when it is unset or not UTF-8.
fn supports_color_with(is_terminal: bool, var: impl Fn(&str) -> Option<String>) -> bool {
    let set_and_not_zero = |name: &str| var(name).is_some_and(|value| value != "0");
    if is_forced(&var) {
        return true;
    }
    let term = var("TERM");
    if set_and_not_zero("NO_COLOR")
        || term.as_deref() == Some("dumb")
        || !(is_terminal || set_and_not_zero("IGNORE_IS_TERMINAL"))
    {
        return false;
    }
    var("COLORTERM").is_some()
        || matches!(
            var("TERM_PROGRAM").as_deref(),
            Some("iTerm.app" | "Apple_Terminal")
        )
        || term_supports_ansi(term.as_deref())
        || set_and_not_zero("CLICOLOR")
        || is_ci(&var)
}

/// Whether `FORCE_COLOR` (or, when it is unset, `CLICOLOR_FORCE`) forces colors.
///
/// `FORCE_COLOR` accepts `true`, an empty value or a positive level; `false`
/// and `0` do not force colors.
fn is_forced(var: &impl Fn(&str) -> Option<String>) -> bool {
    match var("FORCE_COLOR") {
        Some(value) => match value.as_str() {
            "true" | "" => true,
            "false" => false,
            level => level.parse::<usize>().map_or(true, |level| level > 0),
        },
        None => var("CLICOLOR_FORCE").is_some_and(|value| value != "0"),
    }
}

/// Whether `TERM` names an ANSI-capable terminal; Windows consoles without
/// `TERM` support ANSI, while `cygwin` there does not.
fn term_supports_ansi(term: Option<&str>) -> bool {
    match term {
        Some(term) => term != "dumb" && !(cfg!(windows) && term == "cygwin"),
        None => cfg!(windows),
    }
}

/// Whether the environment identifies a known CI service.
fn is_ci(var: &impl Fn(&str) -> Option<String>) -> bool {
    matches!(var("CI").as_deref(), Some("true" | "1" | "woodpecker"))
        || var("NODE").is_some_and(|node| node.ends_with("//heroku/node/bin/node"))
        || CI_MARKERS.iter().any(|name| var(name).is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> + use<> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn forced_colors_win_over_everything() {
        for value in ["true", "", "1", "3", "not-a-number"] {
            let var = env_of(&[("FORCE_COLOR", value), ("NO_COLOR", "1"), ("TERM", "dumb")]);
            assert!(supports_color_with(false, var), "FORCE_COLOR={value:?}");
        }
        assert!(supports_color_with(
            false,
            env_of(&[("CLICOLOR_FORCE", "1")])
        ));
    }

    #[test]
    fn force_color_off_values_do_not_force_and_mask_clicolor_force() {
        for value in ["false", "0"] {
            let var = env_of(&[("FORCE_COLOR", value), ("CLICOLOR_FORCE", "1")]);
            assert!(!supports_color_with(false, var), "FORCE_COLOR={value:?}");
        }
        assert!(!supports_color_with(
            false,
            env_of(&[("CLICOLOR_FORCE", "0")])
        ));
    }

    #[test]
    fn no_color_dumb_terminal_and_pipes_disable_colors() {
        let term = ("TERM", "xterm-256color");
        assert!(!supports_color_with(
            true,
            env_of(&[term, ("NO_COLOR", "")])
        ));
        assert!(supports_color_with(
            true,
            env_of(&[term, ("NO_COLOR", "0")])
        ));
        assert!(!supports_color_with(
            true,
            env_of(&[("TERM", "dumb"), ("CI", "true")])
        ));
        assert!(!supports_color_with(false, env_of(&[term])));
        assert!(supports_color_with(
            false,
            env_of(&[term, ("IGNORE_IS_TERMINAL", "1")])
        ));
    }

    #[test]
    fn capable_terminals_enable_colors() {
        assert!(supports_color_with(true, env_of(&[("TERM", "xterm")])));
        assert!(supports_color_with(
            true,
            env_of(&[("COLORTERM", "truecolor")])
        ));
        assert!(supports_color_with(
            true,
            env_of(&[("TERM_PROGRAM", "Apple_Terminal")])
        ));
        assert!(supports_color_with(true, env_of(&[("CLICOLOR", "1")])));
        assert!(!supports_color_with(true, env_of(&[("CLICOLOR", "0")])));
        assert_eq!(supports_color_with(true, env_of(&[])), cfg!(windows));
    }

    #[test]
    fn ci_environments_are_recognised() {
        for pairs in [
            &[("CI", "true")][..],
            &[("CI", "1")],
            &[("CI", "woodpecker")],
            &[("GITHUB_ACTION", "run")],
            &[("JENKINS_URL", "")],
            &[("NODE", "x//heroku/node/bin/node")],
        ] {
            assert!(is_ci(&env_of(pairs)), "{pairs:?}");
        }
        for pairs in [
            &[][..],
            &[("CI", "false")],
            &[("NODE", "/app/.heroku/node/bin/node")],
        ] {
            assert!(!is_ci(&env_of(pairs)), "{pairs:?}");
        }
    }

    #[test]
    fn ci_enables_colors_only_on_terminals() {
        let var = || env_of(&[("CI", "true"), ("TERM_PROGRAM", "other")]);
        if cfg!(windows) {
            return;
        }
        assert!(supports_color_with(true, var()));
        assert!(!supports_color_with(false, var()));
    }

    #[test]
    fn process_query_is_consistent_for_each_stream() {
        // Only checks that the process-level query runs for both streams.
        let _ = supports_color(TerminalStream::Stdout);
        let _ = supports_color(TerminalStream::Stderr);
    }
}
