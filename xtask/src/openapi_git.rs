//! Closed environment for OpenAPI Git provenance reads.

use std::{env, ffi::OsStr, process::Command};

pub fn command() -> Command {
    let mut command = Command::new("git");
    for (name, _) in env::vars_os() {
        if is_git_variable(&name) {
            command.env_remove(name);
        }
    }
    for (name, value) in [
        ("GIT_OPTIONAL_LOCKS", "0"),
        ("GIT_NO_LAZY_FETCH", "1"),
        ("GIT_NO_REPLACE_OBJECTS", "1"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_COUNT", "2"),
        ("GIT_CONFIG_KEY_0", "core.hooksPath"),
        ("GIT_CONFIG_VALUE_0", "/dev/null"),
        ("GIT_CONFIG_KEY_1", "core.fsmonitor"),
        ("GIT_CONFIG_VALUE_1", "false"),
    ] {
        command.env(name, value);
    }
    command
}

fn is_git_variable(name: &OsStr) -> bool {
    name.to_string_lossy().starts_with("GIT_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_covers_routing_objects_and_config() {
        for name in [
            "GIT_DIR",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_CONFIG_COUNT",
        ] {
            assert!(is_git_variable(OsStr::new(name)));
        }
        assert!(!is_git_variable(OsStr::new("PATH")));
    }
}
