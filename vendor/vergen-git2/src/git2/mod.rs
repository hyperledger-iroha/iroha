// Copyright (c) 2022 pud developers
//
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. All files in the project carrying such notice may not be copied,
// modified, or distributed except according to those terms.

#[cfg(test)]
use anyhow::anyhow;
use anyhow::{Error, Result};
use derive_builder::Builder as DeriveBuilder;
use git2_rs::{
    BranchType, Commit, DescribeFormatOptions, DescribeOptions, Reference, Repository,
    StatusOptions,
};
use std::{
    env::{self, VarError},
    path::{Path, PathBuf},
    str::FromStr,
};
use time::{
    format_description::{self, well_known::Iso8601},
    OffsetDateTime, UtcOffset,
};
use vergen_lib::{
    add_default_map_entry, add_map_entry,
    constants::{
        GIT_BRANCH_NAME, GIT_COMMIT_AUTHOR_EMAIL, GIT_COMMIT_AUTHOR_NAME, GIT_COMMIT_COUNT,
        GIT_COMMIT_DATE_NAME, GIT_COMMIT_MESSAGE, GIT_COMMIT_TIMESTAMP_NAME, GIT_DESCRIBE_NAME,
        GIT_DIRTY_NAME, GIT_SHA_NAME,
    },
    AddEntries, CargoRerunIfChanged, CargoRustcEnvMap, CargoWarning, DefaultConfig, VergenKey,
};

/// The `VERGEN_GIT_*` configuration features
///
/// | Variable | Sample |
/// | -------  | ------ |
/// | `VERGEN_GIT_BRANCH` | feature/fun |
/// | `VERGEN_GIT_COMMIT_AUTHOR_EMAIL` | janedoe@email.com |
/// | `VERGEN_GIT_COMMIT_AUTHOR_NAME` | Jane Doe |
/// | `VERGEN_GIT_COMMIT_COUNT` | 330 |
/// | `VERGEN_GIT_COMMIT_DATE` | 2021-02-24 |
/// | `VERGEN_GIT_COMMIT_MESSAGE` | feat: add commit messages |
/// | `VERGEN_GIT_COMMIT_TIMESTAMP` | 2021-02-24T20:55:21+00:00 |
/// | `VERGEN_GIT_DESCRIBE` | 5.0.0-2-gf49246c |
/// | `VERGEN_GIT_SHA` | f49246ce334567bff9f950bfd0f3078184a2738a |
/// | `VERGEN_GIT_DIRTY` | true |
///
/// # Example
///
/// ```
/// # use anyhow::Result;
/// # use vergen_git2::{Emitter, Git2Builder};
/// #
/// # fn main() -> Result<()> {
/// let git2 = Git2Builder::all_git()?;
/// Emitter::default().add_instructions(&git2)?.emit()?;
/// #   Ok(())
/// # }
/// ```
///
/// Override output with your own value
///
/// ```
/// # use anyhow::Result;
/// # use vergen_git2::{Emitter, Git2Builder};
/// #
/// # fn main() -> Result<()> {
/// temp_env::with_var("VERGEN_GIT_BRANCH", Some("this is the branch I want output"), || {
///     let result = || -> Result<()> {
///         let git2 = Git2Builder::all_git()?;
///         Emitter::default().add_instructions(&git2)?.emit()?;
///         Ok(())
///     }();
///     assert!(result.is_ok());
/// });
/// #   Ok(())
/// # }
/// ```
///
#[derive(Clone, Debug, DeriveBuilder, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct Git2 {
    /// An optional path to a repository.
    #[builder(default = "None")]
    repo_path: Option<PathBuf>,
    /// Emit the current git branch
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_BRANCH=<BRANCH_NAME>
    /// ```
    ///
    #[builder(default = "false")]
    branch: bool,
    /// Emit the author email of the most recent commit
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_COMMIT_AUTHOR_EMAIL=<AUTHOR_EMAIL>
    /// ```
    ///
    #[builder(default = "false")]
    commit_author_name: bool,
    /// Emit the author name of the most recent commit
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_COMMIT_AUTHOR_NAME=<AUTHOR_NAME>
    /// ```
    ///
    #[builder(default = "false")]
    commit_author_email: bool,
    /// Emit the total commit count to HEAD
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_COMMIT_COUNT=<COUNT>
    /// ```
    #[builder(default = "false")]
    commit_count: bool,
    /// Emit the commit message of the latest commit
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_COMMIT_MESSAGE=<MESSAGE>
    /// ```
    ///
    #[builder(default = "false")]
    commit_message: bool,
    /// Emit the commit date of the latest commit
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_COMMIT_DATE=<YYYY-MM-DD>
    /// ```
    ///
    #[builder(default = "false")]
    commit_date: bool,
    /// Emit the commit timestamp of the latest commit
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_COMMIT_TIMESTAMP=<YYYY-MM-DDThh:mm:ssZ>
    /// ```
    ///
    #[builder(default = "false")]
    commit_timestamp: bool,
    /// Emit the describe output
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_DESCRIBE=<DESCRIBE>
    /// ```
    ///
    /// Optionally, add the `dirty` or `tags` flag to describe.
    /// See [`git describe`](https://git-scm.com/docs/git-describe#_options) for more details
    ///
    #[builder(default = "false", setter(custom))]
    describe: bool,
    /// Instead of using only the annotated tags, use any tag found in refs/tags namespace.
    #[builder(default = "false", private)]
    describe_tags: bool,
    /// If the working tree has local modification "-dirty" is appended to it.
    #[builder(default = "false", private)]
    describe_dirty: bool,
    /// Only consider tags matching the given glob pattern, excluding the "refs/tags/" prefix.
    #[builder(default = "None", private)]
    describe_match_pattern: Option<&'static str>,
    /// Emit the SHA of the latest commit
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_SHA=<SHA>
    /// ```
    ///
    /// Optionally, add the `short` flag to rev-parse.
    /// See [`git rev-parse`](https://git-scm.com/docs/git-rev-parse#_options_for_output) for more details.
    ///
    #[builder(default = "false", setter(custom))]
    sha: bool,
    /// Shortens the object name to a unique prefix
    #[builder(default = "false", private)]
    sha_short: bool,
    /// Emit the dirty state of the git repository
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_DIRTY=(true|false)
    /// ```
    ///
    /// Optionally, include untracked files when determining the dirty status of the repository.
    ///
    #[builder(default = "false", setter(custom))]
    dirty: bool,
    /// Should we include/ignore untracked files in deciding whether the repository is dirty.
    #[builder(default = "false", private)]
    dirty_include_untracked: bool,
    /// Enable local offset date/timestamp output
    #[builder(default = "false")]
    use_local: bool,
    #[cfg(test)]
    /// Fail
    #[builder(default = "false")]
    fail: bool,
}

impl Git2Builder {
    /// Emit all of the `VERGEN_GIT_*` instructions
    ///
    /// # Errors
    /// The underlying build function can error
    ///
    pub fn all_git() -> Result<Git2> {
        Self::default()
            .branch(true)
            .commit_author_email(true)
            .commit_author_name(true)
            .commit_count(true)
            .commit_date(true)
            .commit_message(true)
            .commit_timestamp(true)
            .describe(false, false, None)
            .sha(false)
            .dirty(false)
            .build()
            .map_err(Into::into)
    }

    /// Convenience method to setup the [`Git2Builder`] with all of the `VERGEN_GIT_*` instructions on
    pub fn all(&mut self) -> &mut Self {
        self.branch(true)
            .commit_author_email(true)
            .commit_author_name(true)
            .commit_count(true)
            .commit_date(true)
            .commit_message(true)
            .commit_timestamp(true)
            .describe(false, false, None)
            .sha(false)
            .dirty(false)
    }

    /// Emit the describe output
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_DESCRIBE=<DESCRIBE>
    /// ```
    ///
    /// Optionally, add the `dirty` or `tags` flag to describe.
    /// See [`git describe`](https://git-scm.com/docs/git-describe#_options) for more details
    ///
    pub fn describe(
        &mut self,
        tags: bool,
        dirty: bool,
        matches: Option<&'static str>,
    ) -> &mut Self {
        self.describe = Some(true);
        let _ = self.describe_tags(tags);
        let _ = self.describe_dirty(dirty);
        let _ = self.describe_match_pattern(matches);
        self
    }

    /// Emit the dirty state of the git repository
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_DIRTY=(true|false)
    /// ```
    ///
    /// Optionally, include untracked files when determining the dirty status of the repository.
    ///
    pub fn dirty(&mut self, include_untracked: bool) -> &mut Self {
        self.dirty = Some(true);
        let _ = self.dirty_include_untracked(include_untracked);
        self
    }

    /// Emit the SHA of the latest commit
    ///
    /// ```text
    /// cargo:rustc-env=VERGEN_GIT_SHA=<SHA>
    /// ```
    ///
    /// Optionally, add the `short` flag to rev-parse.
    /// See [`git rev-parse`](https://git-scm.com/docs/git-rev-parse#_options_for_output) for more details.
    ///
    pub fn sha(&mut self, short: bool) -> &mut Self {
        self.sha = Some(true);
        let _ = self.sha_short(short);
        self
    }
}

impl Git2 {
    fn any(&self) -> bool {
        self.branch
            || self.commit_author_email
            || self.commit_author_name
            || self.commit_count
            || self.commit_date
            || self.commit_message
            || self.commit_timestamp
            || self.describe
            || self.sha
            || self.dirty
    }

    /// Use the repository location at the given path to determine the git instruction output.
    pub fn at_path(&mut self, path: PathBuf) -> &mut Self {
        self.repo_path = Some(path);
        self
    }

    #[cfg(test)]
    pub(crate) fn fail(&mut self) -> &mut Self {
        self.fail = true;
        self
    }

    #[cfg(not(test))]
    fn add_entries(
        &self,
        idempotent: bool,
        cargo_rustc_env: &mut CargoRustcEnvMap,
        cargo_rerun_if_changed: &mut CargoRerunIfChanged,
        cargo_warning: &mut CargoWarning,
    ) -> Result<()> {
        self.inner_add_entries(
            idempotent,
            cargo_rustc_env,
            cargo_rerun_if_changed,
            cargo_warning,
        )
    }

    #[cfg(test)]
    fn add_entries(
        &self,
        idempotent: bool,
        cargo_rustc_env: &mut CargoRustcEnvMap,
        cargo_rerun_if_changed: &mut CargoRerunIfChanged,
        cargo_warning: &mut CargoWarning,
    ) -> Result<()> {
        if self.fail {
            return Err(anyhow!("failed to create entries"));
        }
        self.inner_add_entries(
            idempotent,
            cargo_rustc_env,
            cargo_rerun_if_changed,
            cargo_warning,
        )
    }

    #[allow(clippy::too_many_lines)]
    fn inner_add_entries(
        &self,
        idempotent: bool,
        cargo_rustc_env: &mut CargoRustcEnvMap,
        cargo_rerun_if_changed: &mut CargoRerunIfChanged,
        cargo_warning: &mut CargoWarning,
    ) -> Result<()> {
        let repo_dir = if let Some(path) = &self.repo_path {
            path.clone()
        } else {
            env::current_dir()?
        };
        let repo = Repository::discover(repo_dir)?;
        let ref_head = repo.find_reference("HEAD")?;
        let git_path = repo.path().to_path_buf();
        let commit = ref_head.peel_to_commit()?;

        if !idempotent && self.any() {
            Self::add_rerun_if_changed(&ref_head, &git_path, cargo_rerun_if_changed);
        }

        if self.branch {
            if let Ok(_value) = env::var(GIT_BRANCH_NAME) {
                add_default_map_entry(VergenKey::GitBranch, cargo_rustc_env, cargo_warning);
            } else {
                Self::add_branch_name(false, &repo, cargo_rustc_env, cargo_warning)?;
            }
        }

        if self.commit_author_email {
            if let Ok(_value) = env::var(GIT_COMMIT_AUTHOR_EMAIL) {
                add_default_map_entry(
                    VergenKey::GitCommitAuthorEmail,
                    cargo_rustc_env,
                    cargo_warning,
                );
            } else {
                Self::add_opt_value(
                    commit.author().email(),
                    VergenKey::GitCommitAuthorEmail,
                    cargo_rustc_env,
                    cargo_warning,
                );
            }
        }

        if self.commit_author_name {
            if let Ok(_value) = env::var(GIT_COMMIT_AUTHOR_NAME) {
                add_default_map_entry(
                    VergenKey::GitCommitAuthorName,
                    cargo_rustc_env,
                    cargo_warning,
                );
            } else {
                Self::add_opt_value(
                    commit.author().name(),
                    VergenKey::GitCommitAuthorName,
                    cargo_rustc_env,
                    cargo_warning,
                );
            }
        }

        if self.commit_count {
            if let Ok(_value) = env::var(GIT_COMMIT_COUNT) {
                add_default_map_entry(VergenKey::GitCommitCount, cargo_rustc_env, cargo_warning);
            } else {
                Self::add_commit_count(false, &repo, cargo_rustc_env, cargo_warning);
            }
        }

        self.add_git_timestamp_entries(&commit, idempotent, cargo_rustc_env, cargo_warning)?;

        if self.commit_message {
            if let Ok(_value) = env::var(GIT_COMMIT_MESSAGE) {
                add_default_map_entry(VergenKey::GitCommitMessage, cargo_rustc_env, cargo_warning);
            } else {
                Self::add_opt_value(
                    commit.message(),
                    VergenKey::GitCommitMessage,
                    cargo_rustc_env,
                    cargo_warning,
                );
            }
        }

        if self.sha {
            if let Ok(_value) = env::var(GIT_SHA_NAME) {
                add_default_map_entry(VergenKey::GitSha, cargo_rustc_env, cargo_warning);
            } else if self.sha_short {
                let obj = repo.revparse_single("HEAD")?;
                Self::add_opt_value(
                    obj.short_id()?.as_str(),
                    VergenKey::GitSha,
                    cargo_rustc_env,
                    cargo_warning,
                );
            } else {
                add_map_entry(VergenKey::GitSha, commit.id().to_string(), cargo_rustc_env);
            }
        }

        if self.dirty {
            if let Ok(_value) = env::var(GIT_DIRTY_NAME) {
                add_default_map_entry(VergenKey::GitDirty, cargo_rustc_env, cargo_warning);
            } else {
                let mut status_options = StatusOptions::new();

                _ = status_options.include_untracked(self.dirty_include_untracked);
                let statuses = repo.statuses(Some(&mut status_options))?;

                let n_dirty = statuses
                    .iter()
                    .filter(|each_status| !each_status.status().is_ignored())
                    .count();

                add_map_entry(
                    VergenKey::GitDirty,
                    format!("{}", n_dirty > 0),
                    cargo_rustc_env,
                );
            }
        }

        if self.describe {
            if let Ok(_value) = env::var(GIT_DESCRIBE_NAME) {
                add_default_map_entry(VergenKey::GitDescribe, cargo_rustc_env, cargo_warning);
            } else {
                let mut describe_opts = DescribeOptions::new();
                let mut format_opts = DescribeFormatOptions::new();

                _ = describe_opts.show_commit_oid_as_fallback(true);

                if self.describe_dirty {
                    _ = format_opts.dirty_suffix("-dirty");
                }

                if self.describe_tags {
                    _ = describe_opts.describe_tags();
                }

                if let Some(pattern) = self.describe_match_pattern {
                    _ = describe_opts.pattern(pattern);
                }

                let describe = repo
                    .describe(&describe_opts)
                    .map(|x| x.format(Some(&format_opts)).map_err(Error::from))??;
                add_map_entry(VergenKey::GitDescribe, describe, cargo_rustc_env);
            }
        }

        Ok(())
    }

    fn add_rerun_if_changed(
        ref_head: &Reference<'_>,
        git_path: &Path,
        cargo_rerun_if_changed: &mut CargoRerunIfChanged,
    ) {
        // Setup the head path
        let mut head_path = git_path.to_path_buf();
        head_path.push("HEAD");

        // Check whether the path exists in the filesystem before emitting it
        if head_path.exists() {
            cargo_rerun_if_changed.push(format!("{}", head_path.display()));
        }

        if let Ok(resolved) = ref_head.resolve() {
            if let Some(name) = resolved.name() {
                let ref_path = git_path.to_path_buf();
                let path = ref_path.join(name);
                // Check whether the path exists in the filesystem before emitting it
                if path.exists() {
                    cargo_rerun_if_changed.push(format!("{}", path.display()));
                }
            }
        }
    }

    fn add_branch_name(
        add_default: bool,
        repo: &Repository,
        cargo_rustc_env: &mut CargoRustcEnvMap,
        cargo_warning: &mut CargoWarning,
    ) -> Result<()> {
        if repo.head_detached()? {
            if add_default {
                add_default_map_entry(VergenKey::GitBranch, cargo_rustc_env, cargo_warning);
            } else {
                add_map_entry(VergenKey::GitBranch, "HEAD", cargo_rustc_env);
            }
        } else {
            let locals = repo.branches(Some(BranchType::Local))?;
            let mut found_head = false;
            for (local, _bt) in locals.filter_map(std::result::Result::ok) {
                if local.is_head() {
                    if let Some(name) = local.name()? {
                        add_map_entry(VergenKey::GitBranch, name, cargo_rustc_env);
                        found_head = !add_default;
                        break;
                    }
                }
            }
            if !found_head {
                add_default_map_entry(VergenKey::GitBranch, cargo_rustc_env, cargo_warning);
            }
        }
        Ok(())
    }

    #[allow(clippy::map_unwrap_or)]
    fn add_opt_value(
        value: Option<&str>,
        key: VergenKey,
        cargo_rustc_env: &mut CargoRustcEnvMap,
        cargo_warning: &mut CargoWarning,
    ) {
        value
            .map(|val| add_map_entry(key, val, cargo_rustc_env))
            .unwrap_or_else(|| add_default_map_entry(key, cargo_rustc_env, cargo_warning));
    }

    fn add_commit_count(
        add_default: bool,
        repo: &Repository,
        cargo_rustc_env: &mut CargoRustcEnvMap,
        cargo_warning: &mut CargoWarning,
    ) {
        let key = VergenKey::GitCommitCount;
        if !add_default {
            if let Ok(mut revwalk) = repo.revwalk() {
                if revwalk.push_head().is_ok() {
                    add_map_entry(key, revwalk.count().to_string(), cargo_rustc_env);
                    return;
                }
            }
        }
        add_default_map_entry(key, cargo_rustc_env, cargo_warning);
    }

    fn add_git_timestamp_entries(
        &self,
        commit: &Commit<'_>,
        idempotent: bool,
        cargo_rustc_env: &mut CargoRustcEnvMap,
        cargo_warning: &mut CargoWarning,
    ) -> Result<()> {
        let (sde, ts) = match env::var("SOURCE_DATE_EPOCH") {
            Ok(v) => (
                true,
                OffsetDateTime::from_unix_timestamp(i64::from_str(&v)?)?,
            ),
            Err(VarError::NotPresent) => self.compute_local_offset(commit)?,
            Err(e) => return Err(e.into()),
        };

        if let Ok(_value) = env::var(GIT_COMMIT_DATE_NAME) {
            add_default_map_entry(VergenKey::GitCommitDate, cargo_rustc_env, cargo_warning);
        } else {
            self.add_git_date_entry(idempotent, sde, &ts, cargo_rustc_env, cargo_warning)?;
        }
        if let Ok(_value) = env::var(GIT_COMMIT_TIMESTAMP_NAME) {
            add_default_map_entry(
                VergenKey::GitCommitTimestamp,
                cargo_rustc_env,
                cargo_warning,
            );
        } else {
            self.add_git_timestamp_entry(idempotent, sde, &ts, cargo_rustc_env, cargo_warning)?;
        }
        Ok(())
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    // this in not included in coverage, because on *nix the local offset is always unsafe
    fn compute_local_offset(&self, commit: &Commit<'_>) -> Result<(bool, OffsetDateTime)> {
        let no_offset = OffsetDateTime::from_unix_timestamp(commit.time().seconds())?;
        if self.use_local {
            let local = UtcOffset::local_offset_at(no_offset)?;
            let local_offset = no_offset.checked_to_offset(local).unwrap_or(no_offset);
            Ok((false, local_offset))
        } else {
            Ok((false, no_offset))
        }
    }

    fn add_git_date_entry(
        &self,
        idempotent: bool,
        source_date_epoch: bool,
        ts: &OffsetDateTime,
        cargo_rustc_env: &mut CargoRustcEnvMap,
        cargo_warning: &mut CargoWarning,
    ) -> Result<()> {
        if self.commit_date {
            if idempotent && !source_date_epoch {
                add_default_map_entry(VergenKey::GitCommitDate, cargo_rustc_env, cargo_warning);
            } else {
                let format = format_description::parse("[year]-[month]-[day]")?;
                add_map_entry(
                    VergenKey::GitCommitDate,
                    ts.format(&format)?,
                    cargo_rustc_env,
                );
            }
        }
        Ok(())
    }

    fn add_git_timestamp_entry(
        &self,
        idempotent: bool,
        source_date_epoch: bool,
        ts: &OffsetDateTime,
        cargo_rustc_env: &mut CargoRustcEnvMap,
        cargo_warning: &mut CargoWarning,
    ) -> Result<()> {
        if self.commit_timestamp {
            if idempotent && !source_date_epoch {
                add_default_map_entry(
                    VergenKey::GitCommitTimestamp,
                    cargo_rustc_env,
                    cargo_warning,
                );
            } else {
                add_map_entry(
                    VergenKey::GitCommitTimestamp,
                    ts.format(&Iso8601::DEFAULT)?,
                    cargo_rustc_env,
                );
            }
        }
        Ok(())
    }
}

impl AddEntries for Git2 {
    fn add_map_entries(
        &self,
        idempotent: bool,
        cargo_rustc_env: &mut CargoRustcEnvMap,
        cargo_rerun_if_changed: &mut CargoRerunIfChanged,
        cargo_warning: &mut CargoWarning,
    ) -> Result<()> {
        if self.any() {
            self.add_entries(
                idempotent,
                cargo_rustc_env,
                cargo_rerun_if_changed,
                cargo_warning,
            )?;
        }
        Ok(())
    }

    fn add_default_entries(
        &self,
        config: &DefaultConfig,
        cargo_rustc_env_map: &mut CargoRustcEnvMap,
        cargo_rerun_if_changed: &mut CargoRerunIfChanged,
        cargo_warning: &mut CargoWarning,
    ) -> Result<()> {
        if *config.fail_on_error() {
            let error = Error::msg(format!("{}", config.error()));
            Err(error)
        } else {
            // Clear any previous warnings.  This should be it.
            cargo_warning.clear();
            cargo_rerun_if_changed.clear();

            cargo_warning.push(format!("{}", config.error()));

            if self.branch {
                add_default_map_entry(VergenKey::GitBranch, cargo_rustc_env_map, cargo_warning);
            }
            if self.commit_author_email {
                add_default_map_entry(
                    VergenKey::GitCommitAuthorEmail,
                    cargo_rustc_env_map,
                    cargo_warning,
                );
            }
            if self.commit_author_name {
                add_default_map_entry(
                    VergenKey::GitCommitAuthorName,
                    cargo_rustc_env_map,
                    cargo_warning,
                );
            }
            if self.commit_count {
                add_default_map_entry(
                    VergenKey::GitCommitCount,
                    cargo_rustc_env_map,
                    cargo_warning,
                );
            }
            if self.commit_date {
                add_default_map_entry(VergenKey::GitCommitDate, cargo_rustc_env_map, cargo_warning);
            }
            if self.commit_message {
                add_default_map_entry(
                    VergenKey::GitCommitMessage,
                    cargo_rustc_env_map,
                    cargo_warning,
                );
            }
            if self.commit_timestamp {
                add_default_map_entry(
                    VergenKey::GitCommitTimestamp,
                    cargo_rustc_env_map,
                    cargo_warning,
                );
            }
            if self.describe {
                add_default_map_entry(VergenKey::GitDescribe, cargo_rustc_env_map, cargo_warning);
            }
            if self.sha {
                add_default_map_entry(VergenKey::GitSha, cargo_rustc_env_map, cargo_warning);
            }
            if self.dirty {
                add_default_map_entry(VergenKey::GitDirty, cargo_rustc_env_map, cargo_warning);
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::{Git2, Git2Builder};
    use anyhow::Result;
    use git2_rs::Repository;
    use serial_test::serial;
    #[cfg(unix)]
    use std::io::stdout;
    use std::{collections::BTreeMap, env::current_dir, io::Write};
    use test_util::TestRepos;
    #[cfg(unix)]
    use test_util::TEST_MTIME;
    use vergen::Emitter;
    use vergen_lib::{count_idempotent, VergenKey};

    #[test]
    #[serial]
    #[allow(clippy::clone_on_copy, clippy::redundant_clone)]
    fn git2_clone_works() -> Result<()> {
        let git2 = Git2Builder::all_git()?;
        let another = git2.clone();
        assert_eq!(another, git2);
        Ok(())
    }

    #[test]
    #[serial]
    fn git2_debug_works() -> Result<()> {
        let git2 = Git2Builder::all_git()?;
        let mut buf = vec![];
        write!(buf, "{git2:?}")?;
        assert!(!buf.is_empty());
        Ok(())
    }

    #[test]
    #[serial]
    fn git2_default() -> Result<()> {
        let git2 = Git2Builder::default().build()?;
        let emitter = Emitter::default().add_instructions(&git2)?.test_emit();
        assert_eq!(0, emitter.cargo_rustc_env_map().len());
        assert_eq!(0, count_idempotent(emitter.cargo_rustc_env_map()));
        assert_eq!(0, emitter.cargo_warning().len());
        Ok(())
    }

    #[test]
    #[serial]
    fn empty_email_is_default() -> Result<()> {
        let mut cargo_rustc_env = BTreeMap::new();
        let mut cargo_warning = vec![];
        Git2::add_opt_value(
            None,
            VergenKey::GitCommitAuthorEmail,
            &mut cargo_rustc_env,
            &mut cargo_warning,
        );
        assert_eq!(1, cargo_rustc_env.len());
        assert_eq!(1, cargo_warning.len());
        Ok(())
    }

    #[test]
    #[serial]
    fn bad_revwalk_is_default() -> Result<()> {
        let mut cargo_rustc_env = BTreeMap::new();
        let mut cargo_warning = vec![];
        let repo = Repository::discover(current_dir()?)?;
        Git2::add_commit_count(true, &repo, &mut cargo_rustc_env, &mut cargo_warning);
        assert_eq!(1, cargo_rustc_env.len());
        assert_eq!(1, cargo_warning.len());
        Ok(())
    }

    #[test]
    #[serial]
    fn head_not_found_is_default() -> Result<()> {
        let test_repo = TestRepos::new(false, false, false)?;
        let mut map = BTreeMap::new();
        let mut cargo_warning = vec![];
        let repo = Repository::discover(current_dir()?)?;
        Git2::add_branch_name(true, &repo, &mut map, &mut cargo_warning)?;
        assert_eq!(1, map.len());
        assert_eq!(1, cargo_warning.len());
        let mut map = BTreeMap::new();
        let mut cargo_warning = vec![];
        let repo = Repository::discover(test_repo.path())?;
        Git2::add_branch_name(true, &repo, &mut map, &mut cargo_warning)?;
        assert_eq!(1, map.len());
        assert_eq!(1, cargo_warning.len());
        Ok(())
    }

    #[test]
    #[serial]
    fn git_all_idempotent() -> Result<()> {
        let git2 = Git2Builder::all_git()?;
        let emitter = Emitter::default()
            .idempotent()
            .add_instructions(&git2)?
            .test_emit();
        assert_eq!(10, emitter.cargo_rustc_env_map().len());
        assert_eq!(2, count_idempotent(emitter.cargo_rustc_env_map()));
        assert_eq!(2, emitter.cargo_warning().len());
        Ok(())
    }

    #[test]
    #[serial]
    fn git_all_shallow_clone() -> Result<()> {
        let repo = TestRepos::new(false, false, true)?;
        let mut git2 = Git2Builder::all_git()?;
        let _ = git2.at_path(repo.path());
        let emitter = Emitter::default().add_instructions(&git2)?.test_emit();
        assert_eq!(10, emitter.cargo_rustc_env_map().len());
        assert_eq!(0, count_idempotent(emitter.cargo_rustc_env_map()));
        assert_eq!(0, emitter.cargo_warning().len());
        Ok(())
    }

    #[test]
    #[serial]
    fn git_all_idempotent_no_warn() -> Result<()> {
        let git2 = Git2Builder::all_git()?;
        let emitter = Emitter::default()
            .idempotent()
            .quiet()
            .add_instructions(&git2)?
            .test_emit();

        assert_eq!(10, emitter.cargo_rustc_env_map().len());
        assert_eq!(2, count_idempotent(emitter.cargo_rustc_env_map()));
        assert_eq!(2, emitter.cargo_warning().len());
        Ok(())
    }

    #[test]
    #[serial]
    fn git_all() -> Result<()> {
        let git2 = Git2Builder::all_git()?;
        let emitter = Emitter::default().add_instructions(&git2)?.test_emit();
        assert_eq!(10, emitter.cargo_rustc_env_map().len());
        assert_eq!(0, count_idempotent(emitter.cargo_rustc_env_map()));
        assert_eq!(0, emitter.cargo_warning().len());
        Ok(())
    }

    #[test]
    #[serial]
    fn git_error_fails() -> Result<()> {
        let mut git2 = Git2Builder::all_git()?;
        let _ = git2.fail();
        assert!(Emitter::default()
            .fail_on_error()
            .add_instructions(&git2)
            .is_err());
        Ok(())
    }

    #[test]
    #[serial]
    fn git_error_defaults() -> Result<()> {
        let mut git2 = Git2Builder::all_git()?;
        let _ = git2.fail();
        let emitter = Emitter::default().add_instructions(&git2)?.test_emit();
        assert_eq!(10, emitter.cargo_rustc_env_map().len());
        assert_eq!(10, count_idempotent(emitter.cargo_rustc_env_map()));
        assert_eq!(11, emitter.cargo_warning().len());
        Ok(())
    }

    #[test]
    #[serial]
    fn source_date_epoch_works() {
        temp_env::with_var("SOURCE_DATE_EPOCH", Some("1671809360"), || {
            let result = || -> Result<()> {
                let mut stdout_buf = vec![];
                let gix = Git2Builder::default()
                    .commit_date(true)
                    .commit_timestamp(true)
                    .build()?;
                _ = Emitter::new()
                    .idempotent()
                    .add_instructions(&gix)?
                    .emit_to(&mut stdout_buf)?;
                let output = String::from_utf8_lossy(&stdout_buf);
                for (idx, line) in output.lines().enumerate() {
                    if idx == 0 {
                        assert_eq!("cargo:rustc-env=VERGEN_GIT_COMMIT_DATE=2022-12-23", line);
                    } else if idx == 1 {
                        assert_eq!(
                            "cargo:rustc-env=VERGEN_GIT_COMMIT_TIMESTAMP=2022-12-23T15:29:20.000000000Z",
                            line
                        );
                    }
                }
                Ok(())
            }();
            assert!(result.is_ok());
        });
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn bad_source_date_epoch_fails() {
        use std::ffi::OsStr;
        use std::os::unix::prelude::OsStrExt;

        let source = [0x66, 0x6f, 0x80, 0x6f];
        let os_str = OsStr::from_bytes(&source[..]);
        temp_env::with_var("SOURCE_DATE_EPOCH", Some(os_str), || {
            let result = || -> Result<bool> {
                let mut stdout_buf = vec![];
                let gix = Git2Builder::default().commit_date(true).build()?;
                Emitter::new()
                    .idempotent()
                    .fail_on_error()
                    .add_instructions(&gix)?
                    .emit_to(&mut stdout_buf)
            }();
            assert!(result.is_err());
        });
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn bad_source_date_epoch_defaults() {
        use std::ffi::OsStr;
        use std::os::unix::prelude::OsStrExt;

        let source = [0x66, 0x6f, 0x80, 0x6f];
        let os_str = OsStr::from_bytes(&source[..]);
        temp_env::with_var("SOURCE_DATE_EPOCH", Some(os_str), || {
            let result = || -> Result<bool> {
                let mut stdout_buf = vec![];
                let gix = Git2Builder::default().commit_date(true).build()?;
                Emitter::new()
                    .idempotent()
                    .add_instructions(&gix)?
                    .emit_to(&mut stdout_buf)
            }();
            assert!(result.is_ok());
        });
    }

    #[test]
    #[serial]
    #[cfg(windows)]
    fn bad_source_date_epoch_fails() {
        use std::ffi::OsString;
        use std::os::windows::prelude::OsStringExt;

        let source = [0x0066, 0x006f, 0xD800, 0x006f];
        let os_string = OsString::from_wide(&source[..]);
        let os_str = os_string.as_os_str();
        temp_env::with_var("SOURCE_DATE_EPOCH", Some(os_str), || {
            let result = || -> Result<bool> {
                let mut stdout_buf = vec![];
                let gix = Git2Builder::default().commit_date(true).build()?;
                Emitter::new()
                    .fail_on_error()
                    .idempotent()
                    .add_instructions(&gix)?
                    .emit_to(&mut stdout_buf)
            }();
            assert!(result.is_err());
        });
    }

    #[test]
    #[serial]
    #[cfg(windows)]
    fn bad_source_date_epoch_defaults() {
        use std::ffi::OsString;
        use std::os::windows::prelude::OsStringExt;

        let source = [0x0066, 0x006f, 0xD800, 0x006f];
        let os_string = OsString::from_wide(&source[..]);
        let os_str = os_string.as_os_str();
        temp_env::with_var("SOURCE_DATE_EPOCH", Some(os_str), || {
            let result = || -> Result<bool> {
                let mut stdout_buf = vec![];
                let gix = Git2Builder::default().commit_date(true).build()?;
                Emitter::new()
                    .idempotent()
                    .add_instructions(&gix)?
                    .emit_to(&mut stdout_buf)
            }();
            assert!(result.is_ok());
        });
    }

    #[test]
    #[serial]
    #[cfg(unix)]
    fn git_no_index_update() -> Result<()> {
        let repo = TestRepos::new(true, true, false)?;
        repo.set_index_magic_mtime()?;

        let mut git2 = Git2Builder::default()
            .all()
            .describe(true, true, None)
            .build()?;
        let _ = git2.at_path(repo.path());
        let failed = Emitter::default()
            .add_instructions(&git2)?
            .emit_to(&mut stdout())?;
        assert!(!failed);

        assert_eq!(*TEST_MTIME, repo.get_index_magic_mtime()?);
        Ok(())
    }
}
