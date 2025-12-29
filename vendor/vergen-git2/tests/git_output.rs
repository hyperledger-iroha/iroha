mod test_git_git2 {
    use anyhow::Result;
    use git2_rs::Repository;
    use lazy_static::lazy_static;
    use regex::Regex;
    use serial_test::serial;
    use std::env::{current_dir, temp_dir};
    use temp_env::with_var;
    use vergen_git2::{Emitter, Git2Builder};

    use test_util::TestRepos;

    lazy_static! {
        static ref GIT_BRANCH_RE_STR: &'static str = r"cargo:rustc-env=VERGEN_GIT_BRANCH=.*";
        static ref GIT_CAE_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_AUTHOR_EMAIL=\S+@\S+";
        static ref GIT_CAN_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_AUTHOR_NAME=.*";
        static ref GIT_CC_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_COUNT=([0-9]+)";
        static ref GIT_CD_RE_STR: &'static str = r"cargo:rustc-env=VERGEN_GIT_COMMIT_DATE=([0-9]+)-(0[1-9]|1[012])-(0[1-9]|[12][0-9]|3[01])";
        static ref GIT_CD_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_DATE=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_CM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_MESSAGE=[\s\S]+";
        static ref GIT_CT_RE_STR: &'static str = r"cargo:rustc-env=VERGEN_GIT_COMMIT_TIMESTAMP=([0-9]+)-(0[1-9]|1[012])-(0[1-9]|[12][0-9]|3[01])[Tt]([01][0-9]|2[0-3]):([0-5][0-9]):([0-5][0-9]|60)(\.[0-9]+)?(([Zz])|([\+|\-]([01][0-9]|2[0-3]):[0-5][0-9]))";
        static ref GIT_CT_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_TIMESTAMP=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_DESCRIBE_RE_STR: &'static str = r"cargo:rustc-env=VERGEN_GIT_DESCRIBE=.*";
        static ref GIT_SHA_RE_STR: &'static str = r"cargo:rustc-env=VERGEN_GIT_SHA=[0-9a-f]{40}";
        static ref GIT_SHORT_SHA_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_SHA=[0-9a-f]{7}";
        static ref GIT_DIRTY_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_DIRTY=(true|false)";
        static ref GIT_BRANCH_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_BRANCH=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_COMMIT_AUTHOR_EMAIL_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_AUTHOR_EMAIL=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_COMMIT_AUTHOR_NAME_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_AUTHOR_NAME=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_COMMIT_COUNT_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_COUNT=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_COMMIT_DATE_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_DATE=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_COMMIT_MESSAGE_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_MESSAGE=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_COMMIT_TIMESTAMP_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_COMMIT_TIMESTAMP=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_DESCRIBE_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_DESCRIBE=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_SHA_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_SHA=VERGEN_IDEMPOTENT_OUTPUT";
        static ref GIT_DIRTY_IDEM_RE_STR: &'static str =
            r"cargo:rustc-env=VERGEN_GIT_DIRTY=VERGEN_IDEMPOTENT_OUTPUT";
        static ref WARNINGS_RERUN_RE_STR: &'static str = r"cargo:warning=(.*?)
cargo:warning=VERGEN_GIT_BRANCH set to default
cargo:warning=VERGEN_GIT_COMMIT_AUTHOR_EMAIL set to default
cargo:warning=VERGEN_GIT_COMMIT_AUTHOR_NAME set to default
cargo:warning=VERGEN_GIT_COMMIT_COUNT set to default
cargo:warning=VERGEN_GIT_COMMIT_DATE set to default
cargo:warning=VERGEN_GIT_COMMIT_MESSAGE set to default
cargo:warning=VERGEN_GIT_COMMIT_TIMESTAMP set to default
cargo:warning=VERGEN_GIT_DESCRIBE set to default
cargo:warning=VERGEN_GIT_SHA set to default
cargo:warning=VERGEN_GIT_DIRTY set to default
cargo:rerun-if-changed=build.rs
cargo:rerun-if-env-changed=VERGEN_IDEMPOTENT
cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH";
        static ref GIT_REGEX_INST: Regex = {
            let re_str = [
                *GIT_BRANCH_RE_STR,
                *GIT_CAE_RE_STR,
                *GIT_CAN_RE_STR,
                *GIT_CC_RE_STR,
                *GIT_CD_RE_STR,
                *GIT_CM_RE_STR,
                *GIT_CT_RE_STR,
                *GIT_DESCRIBE_RE_STR,
                *GIT_SHA_RE_STR,
                *GIT_DIRTY_RE_STR,
            ]
            .join("\n");
            Regex::new(&re_str).unwrap()
        };
        static ref GIT_REGEX_SHORT_INST: Regex = {
            let re_str = [
                *GIT_BRANCH_RE_STR,
                *GIT_CAE_RE_STR,
                *GIT_CAN_RE_STR,
                *GIT_CC_RE_STR,
                *GIT_CD_RE_STR,
                *GIT_CM_RE_STR,
                *GIT_CT_RE_STR,
                *GIT_DESCRIBE_RE_STR,
                *GIT_SHORT_SHA_RE_STR,
                *GIT_DIRTY_RE_STR,
            ]
            .join("\n");
            Regex::new(&re_str).unwrap()
        };
        static ref GIT_REGEX_IDEM_INST: Regex = {
            let re_str = [
                *GIT_BRANCH_RE_STR,
                *GIT_CAE_RE_STR,
                *GIT_CAN_RE_STR,
                *GIT_CC_RE_STR,
                *GIT_CD_IDEM_RE_STR,
                *GIT_CM_RE_STR,
                *GIT_CT_IDEM_RE_STR,
                *GIT_DESCRIBE_RE_STR,
                *GIT_SHA_RE_STR,
                *GIT_DIRTY_RE_STR,
            ]
            .join("\n");
            Regex::new(&re_str).unwrap()
        };
        static ref ALL_IDEM_OUTPUT: Regex = {
            let re_str = [
                *GIT_BRANCH_IDEM_RE_STR,
                *GIT_COMMIT_AUTHOR_EMAIL_IDEM_RE_STR,
                *GIT_COMMIT_AUTHOR_NAME_IDEM_RE_STR,
                *GIT_COMMIT_COUNT_IDEM_RE_STR,
                *GIT_COMMIT_DATE_IDEM_RE_STR,
                *GIT_COMMIT_MESSAGE_IDEM_RE_STR,
                *GIT_COMMIT_TIMESTAMP_IDEM_RE_STR,
                *GIT_DESCRIBE_IDEM_RE_STR,
                *GIT_SHA_IDEM_RE_STR,
                *GIT_DIRTY_IDEM_RE_STR,
                *WARNINGS_RERUN_RE_STR,
            ]
            .join("\n");
            Regex::new(&re_str).unwrap()
        };
    }

    fn repo_exists() -> Result<bool> {
        let curr_dir = current_dir()?;
        let _repo = Repository::discover(curr_dir)?;
        Ok(true)
    }

    #[test]
    #[serial]
    fn git_all_output_idempotent() -> Result<()> {
        let mut stdout_buf = vec![];
        let mut git2 = Git2Builder::all_git()?;
        let _ = git2.at_path(temp_dir());
        let failed = Emitter::default()
            .add_instructions(&git2)?
            .emit_to(&mut stdout_buf)?;
        let output = String::from_utf8_lossy(&stdout_buf);
        assert!(!failed);
        assert!(ALL_IDEM_OUTPUT.is_match(&output));
        Ok(())
    }

    #[test]
    #[serial]
    fn git_all_output_default_dir() -> Result<()> {
        let mut stdout_buf = vec![];
        let git2 = Git2Builder::all_git()?;
        let failed = Emitter::default()
            .add_instructions(&git2)?
            .emit_to(&mut stdout_buf)?;
        let output = String::from_utf8_lossy(&stdout_buf);
        assert!(!failed);
        assert!(repo_exists().is_ok());
        assert!(GIT_REGEX_INST.is_match(&output));
        Ok(())
    }

    #[test]
    #[serial]
    fn git_all_flags_test_repo() -> Result<()> {
        let repo = TestRepos::new(true, false, false)?;
        let mut stdout_buf = vec![];
        let mut git2 = Git2Builder::default()
            .all()
            .describe(true, false, None)
            .sha(true)
            .build()?;
        let _ = git2.at_path(repo.path());
        let failed = Emitter::default()
            .add_instructions(&git2)?
            .emit_to(&mut stdout_buf)?;
        assert!(!failed);
        let output = String::from_utf8_lossy(&stdout_buf);
        eprintln!("BLAH: {output}");
        assert!(GIT_REGEX_SHORT_INST.is_match(&output));
        Ok(())
    }

    #[test]
    #[serial]
    fn git_all_flags_test_repo_local() -> Result<()> {
        let repo = TestRepos::new(true, false, false)?;
        let mut stdout_buf = vec![];
        let mut git2 = Git2Builder::default()
            .all()
            .describe(true, false, None)
            .sha(true)
            .use_local(true)
            .build()?;
        let _ = git2.at_path(repo.path());
        let result = || -> Result<bool> {
            let failed = Emitter::default()
                .fail_on_error()
                .add_instructions(&git2)?
                .emit_to(&mut stdout_buf)?;
            Ok(failed)
        }();

        check_local_result(result, &stdout_buf);
        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn check_local_result(result: Result<bool>, stdout_buf: &[u8]) {
        assert!(result.is_ok());
        let output = String::from_utf8_lossy(stdout_buf);
        assert!(GIT_REGEX_SHORT_INST.is_match(&output));
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn check_local_result(result: Result<bool>, stdout_buf: &[u8]) {
        assert!(result.is_ok());
        let output = String::from_utf8_lossy(stdout_buf);
        assert!(GIT_REGEX_SHORT_INST.is_match(&output));
    }

    #[test]
    #[serial]
    fn git_all_output_test_repo() -> Result<()> {
        let repo = TestRepos::new(true, true, false)?;
        let mut stdout_buf = vec![];
        let mut git2 = Git2Builder::default()
            .all()
            .describe(true, false, None)
            .build()?;
        let _ = git2.at_path(repo.path());
        let failed = Emitter::default()
            .add_instructions(&git2)?
            .emit_to(&mut stdout_buf)?;
        assert!(!failed);
        let output = String::from_utf8_lossy(&stdout_buf);
        assert!(GIT_REGEX_INST.is_match(&output));
        Ok(())
    }

    #[test]
    #[serial]
    fn git_all_describe_all_test_repo() -> Result<()> {
        let repo = TestRepos::new(true, true, false)?;
        let mut stdout_buf = vec![];
        let mut git2 = Git2Builder::default()
            .all()
            .describe(true, true, Some("0.1.0"))
            .build()?;
        let _ = git2.at_path(repo.path());
        let failed = Emitter::default()
            .add_instructions(&git2)?
            .emit_to(&mut stdout_buf)?;
        assert!(!failed);
        let output = String::from_utf8_lossy(&stdout_buf);
        assert!(GIT_REGEX_INST.is_match(&output));
        Ok(())
    }

    #[test]
    #[serial]
    fn git_emit_at_test_repo() -> Result<()> {
        let repo = TestRepos::new(true, false, false)?;
        let mut git2 = Git2Builder::default()
            .all()
            .describe(true, false, None)
            .sha(true)
            .build()?;
        let _ = git2.at_path(repo.path());
        assert!(Emitter::default().add_instructions(&git2)?.emit().is_ok());
        Ok(())
    }

    mod git_dirty {
        use anyhow::Result;
        use serial_test::serial;
        use test_util::TestRepos;
        use vergen_git2::{Emitter, Git2Builder};

        const GIT_DIRTY_TRUE_OUTPUT: &str = r"cargo:rustc-env=VERGEN_GIT_DIRTY=true";
        const GIT_DIRTY_FALSE_OUTPUT: &str = r"cargo:rustc-env=VERGEN_GIT_DIRTY=false";

        fn strip_reruns(output: &str) -> String {
            let lines: Vec<&str> = output
                .lines()
                .filter(|line| !line.starts_with("cargo:rerun-if"))
                .collect();

            lines.join("\n")
        }

        #[test]
        #[serial]
        fn git_dirty_ignore_untracked_no_modified_no_untracked() -> Result<()> {
            // On a repository with no modified files and no untracked files,
            // dirty should be false.
            let repo = TestRepos::new(false, false, false)?;

            let mut stdout_buf = vec![];
            let mut git2 = Git2Builder::default().dirty(false).build()?;
            let _ = git2.at_path(repo.path());
            let _emitter = Emitter::default()
                .add_instructions(&git2)?
                .emit_to(&mut stdout_buf)?;

            let output = String::from_utf8_lossy(&stdout_buf);
            let stripped_output = strip_reruns(&output);
            assert_eq!(GIT_DIRTY_FALSE_OUTPUT, stripped_output);
            Ok(())
        }

        #[test]
        #[serial]
        fn git_dirty_include_untracked_no_modified_no_untracked() -> Result<()> {
            // On a repository with no modified files and no untracked files,
            // dirty should be false.
            let repo = TestRepos::new(false, false, false)?;

            let mut stdout_buf = vec![];
            let mut git2 = Git2Builder::default().dirty(true).build()?;
            let _ = git2.at_path(repo.path());
            let _emitter = Emitter::default()
                .add_instructions(&git2)?
                .emit_to(&mut stdout_buf)?;

            let output = String::from_utf8_lossy(&stdout_buf);
            let stripped_output = strip_reruns(&output);
            assert_eq!(GIT_DIRTY_FALSE_OUTPUT, stripped_output);
            Ok(())
        }

        #[test]
        #[serial]
        fn git_dirty_ignore_untracked_modified_no_untracked() -> Result<()> {
            // On a repository with modified files and no untracked files,
            // dirty should be true.
            let repo = TestRepos::new(true, false, false)?;

            let mut stdout_buf = vec![];
            let mut git2 = Git2Builder::default().dirty(false).build()?;
            let _ = git2.at_path(repo.path());
            let _emitter = Emitter::default()
                .add_instructions(&git2)?
                .emit_to(&mut stdout_buf)?;

            let output = String::from_utf8_lossy(&stdout_buf);
            let stripped_output = strip_reruns(&output);
            assert_eq!(GIT_DIRTY_TRUE_OUTPUT, stripped_output);
            Ok(())
        }

        #[test]
        #[serial]
        fn git_dirty_include_untracked_modified_no_untracked() -> Result<()> {
            // On a repository with modified files and no untracked files,
            // dirty should be true.
            let repo = TestRepos::new(true, false, false)?;

            let mut stdout_buf = vec![];
            let mut git2 = Git2Builder::default().dirty(true).build()?;
            let _ = git2.at_path(repo.path());
            let _emitter = Emitter::default()
                .add_instructions(&git2)?
                .emit_to(&mut stdout_buf)?;

            let output = String::from_utf8_lossy(&stdout_buf);
            let stripped_output = strip_reruns(&output);
            assert_eq!(GIT_DIRTY_TRUE_OUTPUT, stripped_output);
            Ok(())
        }

        #[test]
        #[serial]
        fn git_dirty_ignore_untracked_no_modified_untracked() -> Result<()> {
            // On a repository with no modified files and untracked files,
            // dirty should be false when include_untracked is false.
            let repo = TestRepos::new(false, true, false)?;

            let mut stdout_buf = vec![];
            let mut git2 = Git2Builder::default().dirty(false).build()?;
            let _ = git2.at_path(repo.path());
            let _emitter = Emitter::default()
                .add_instructions(&git2)?
                .emit_to(&mut stdout_buf)?;

            let output = String::from_utf8_lossy(&stdout_buf);
            let stripped_output = strip_reruns(&output);
            assert_eq!(GIT_DIRTY_FALSE_OUTPUT, stripped_output);
            Ok(())
        }

        #[test]
        #[serial]
        fn git_dirty_include_untracked_no_modified_untracked() -> Result<()> {
            // On a repository with no modified files and untracked files,
            // dirty should be true when include_untracked is true.
            let repo = TestRepos::new(false, true, false)?;

            let mut stdout_buf = vec![];
            let mut git2 = Git2Builder::default().dirty(true).build()?;
            let _ = git2.at_path(repo.path());
            let _emitter = Emitter::default()
                .add_instructions(&git2)?
                .emit_to(&mut stdout_buf)?;

            let output = String::from_utf8_lossy(&stdout_buf);
            let stripped_output = strip_reruns(&output);
            assert_eq!(GIT_DIRTY_TRUE_OUTPUT, stripped_output);
            Ok(())
        }

        #[test]
        #[serial]
        fn git_dirty_ignore_untracked_modified_untracked() -> Result<()> {
            // On a repository with modified files and untracked files,
            // dirty should be true.
            let repo = TestRepos::new(true, true, false)?;

            let mut stdout_buf = vec![];
            let mut git2 = Git2Builder::default().dirty(false).build()?;
            let _ = git2.at_path(repo.path());
            let _emitter = Emitter::default()
                .add_instructions(&git2)?
                .emit_to(&mut stdout_buf)?;

            let output = String::from_utf8_lossy(&stdout_buf);
            let stripped_output = strip_reruns(&output);
            assert_eq!(GIT_DIRTY_TRUE_OUTPUT, stripped_output);
            Ok(())
        }

        #[test]
        #[serial]
        fn git_dirty_include_untracked_modified_untracked() -> Result<()> {
            // On a repository with modified files and untracked files,
            // dirty should be true.
            let repo = TestRepos::new(true, true, false)?;

            let mut stdout_buf = vec![];
            let mut git2 = Git2Builder::default().dirty(true).build()?;
            let _ = git2.at_path(repo.path());
            let _emitter = Emitter::default()
                .add_instructions(&git2)?
                .emit_to(&mut stdout_buf)?;

            let output = String::from_utf8_lossy(&stdout_buf);
            let stripped_output = strip_reruns(&output);
            assert_eq!(GIT_DIRTY_TRUE_OUTPUT, stripped_output);
            Ok(())
        }
    }

    #[test]
    #[serial]
    fn git_all_idempotent_output() -> Result<()> {
        let mut stdout_buf = vec![];
        let git2 = Git2Builder::all_git()?;
        let failed = Emitter::default()
            .idempotent()
            .add_instructions(&git2)?
            .emit_to(&mut stdout_buf)?;
        let output = String::from_utf8_lossy(&stdout_buf);
        assert!(!failed);
        assert!(repo_exists().is_ok());
        assert!(GIT_REGEX_IDEM_INST.is_match(&output));
        Ok(())
    }

    #[test]
    #[serial]
    fn git_all_idempotent_output_quiet() -> Result<()> {
        let mut stdout_buf = vec![];
        let git2 = Git2Builder::all_git()?;
        let failed = Emitter::default()
            .idempotent()
            .quiet()
            .add_instructions(&git2)?
            .emit_to(&mut stdout_buf)?;
        let output = String::from_utf8_lossy(&stdout_buf);
        assert!(!failed);
        assert!(repo_exists().is_ok());
        assert!(GIT_REGEX_IDEM_INST.is_match(&output));
        Ok(())
    }

    #[test]
    #[serial]
    fn git_branch_override_works() {
        with_var("VERGEN_GIT_BRANCH", Some("this is a bad date"), || {
            let result = || -> Result<()> {
                let mut stdout_buf = vec![];
                let git2 = Git2Builder::all_git()?;
                let _failed = Emitter::default()
                    .add_instructions(&git2)?
                    .emit_to(&mut stdout_buf)?;
                let output = String::from_utf8_lossy(&stdout_buf);
                assert!(output.contains("cargo:rustc-env=VERGEN_GIT_BRANCH=this is a bad date"));
                assert!(output.contains("cargo:warning=VERGEN_GIT_BRANCH overidden"));
                Ok(())
            }();
            assert!(result.is_ok());
        });
    }

    #[test]
    #[serial]
    fn git_commit_author_email_override_works() {
        with_var(
            "VERGEN_GIT_COMMIT_AUTHOR_EMAIL",
            Some("this is a bad date"),
            || {
                let result = || -> Result<()> {
                    let mut stdout_buf = vec![];
                    let git2 = Git2Builder::all_git()?;
                    let _failed = Emitter::default()
                        .add_instructions(&git2)?
                        .emit_to(&mut stdout_buf)?;
                    let output = String::from_utf8_lossy(&stdout_buf);
                    assert!(output.contains(
                        "cargo:rustc-env=VERGEN_GIT_COMMIT_AUTHOR_EMAIL=this is a bad date"
                    ));
                    assert!(
                        output.contains("cargo:warning=VERGEN_GIT_COMMIT_AUTHOR_EMAIL overidden")
                    );
                    Ok(())
                }();
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    #[serial]
    fn git_commit_author_name_override_works() {
        with_var(
            "VERGEN_GIT_COMMIT_AUTHOR_NAME",
            Some("this is a bad date"),
            || {
                let result = || -> Result<()> {
                    let mut stdout_buf = vec![];
                    let git2 = Git2Builder::all_git()?;
                    let _failed = Emitter::default()
                        .add_instructions(&git2)?
                        .emit_to(&mut stdout_buf)?;
                    let output = String::from_utf8_lossy(&stdout_buf);
                    assert!(output.contains(
                        "cargo:rustc-env=VERGEN_GIT_COMMIT_AUTHOR_NAME=this is a bad date"
                    ));
                    assert!(
                        output.contains("cargo:warning=VERGEN_GIT_COMMIT_AUTHOR_NAME overidden")
                    );
                    Ok(())
                }();
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    #[serial]
    fn git_commit_count_override_works() {
        with_var(
            "VERGEN_GIT_COMMIT_COUNT",
            Some("this is a bad date"),
            || {
                let result = || -> Result<()> {
                    let mut stdout_buf = vec![];
                    let git2 = Git2Builder::all_git()?;
                    let _failed = Emitter::default()
                        .add_instructions(&git2)?
                        .emit_to(&mut stdout_buf)?;
                    let output = String::from_utf8_lossy(&stdout_buf);
                    assert!(output
                        .contains("cargo:rustc-env=VERGEN_GIT_COMMIT_COUNT=this is a bad date"));
                    assert!(output.contains("cargo:warning=VERGEN_GIT_COMMIT_COUNT overidden"));
                    Ok(())
                }();
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    #[serial]
    fn git_commit_date_override_works() {
        with_var("VERGEN_GIT_COMMIT_DATE", Some("this is a bad date"), || {
            let result = || -> Result<()> {
                let mut stdout_buf = vec![];
                let git2 = Git2Builder::all_git()?;
                let _failed = Emitter::default()
                    .add_instructions(&git2)?
                    .emit_to(&mut stdout_buf)?;
                let output = String::from_utf8_lossy(&stdout_buf);
                assert!(
                    output.contains("cargo:rustc-env=VERGEN_GIT_COMMIT_DATE=this is a bad date")
                );
                assert!(output.contains("cargo:warning=VERGEN_GIT_COMMIT_DATE overidden"));
                Ok(())
            }();
            assert!(result.is_ok());
        });
    }

    #[test]
    #[serial]
    fn git_commit_message_override_works() {
        with_var(
            "VERGEN_GIT_COMMIT_MESSAGE",
            Some("this is a bad date"),
            || {
                let result = || -> Result<()> {
                    let mut stdout_buf = vec![];
                    let git2 = Git2Builder::all_git()?;
                    let _failed = Emitter::default()
                        .add_instructions(&git2)?
                        .emit_to(&mut stdout_buf)?;
                    let output = String::from_utf8_lossy(&stdout_buf);
                    assert!(output
                        .contains("cargo:rustc-env=VERGEN_GIT_COMMIT_MESSAGE=this is a bad date"));
                    assert!(output.contains("cargo:warning=VERGEN_GIT_COMMIT_MESSAGE overidden"));
                    Ok(())
                }();
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    #[serial]
    fn git_commit_timestamp_override_works() {
        with_var(
            "VERGEN_GIT_COMMIT_TIMESTAMP",
            Some("this is a bad date"),
            || {
                let result = || -> Result<()> {
                    let mut stdout_buf = vec![];
                    let git2 = Git2Builder::all_git()?;
                    let _failed = Emitter::default()
                        .add_instructions(&git2)?
                        .emit_to(&mut stdout_buf)?;
                    let output = String::from_utf8_lossy(&stdout_buf);
                    assert!(output.contains(
                        "cargo:rustc-env=VERGEN_GIT_COMMIT_TIMESTAMP=this is a bad date"
                    ));
                    assert!(output.contains("cargo:warning=VERGEN_GIT_COMMIT_TIMESTAMP overidden"));
                    Ok(())
                }();
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    #[serial]
    fn git_describe_override_works() {
        with_var("VERGEN_GIT_DESCRIBE", Some("this is a bad date"), || {
            let result = || -> Result<()> {
                let mut stdout_buf = vec![];
                let git2 = Git2Builder::all_git()?;
                let _failed = Emitter::default()
                    .add_instructions(&git2)?
                    .emit_to(&mut stdout_buf)?;
                let output = String::from_utf8_lossy(&stdout_buf);
                assert!(output.contains("cargo:rustc-env=VERGEN_GIT_DESCRIBE=this is a bad date"));
                assert!(output.contains("cargo:warning=VERGEN_GIT_DESCRIBE overidden"));
                Ok(())
            }();
            assert!(result.is_ok());
        });
    }

    #[test]
    #[serial]
    fn git_sha_override_works() {
        with_var("VERGEN_GIT_SHA", Some("this is a bad date"), || {
            let result = || -> Result<()> {
                let mut stdout_buf = vec![];
                let git2 = Git2Builder::all_git()?;
                let _failed = Emitter::default()
                    .add_instructions(&git2)?
                    .emit_to(&mut stdout_buf)?;
                let output = String::from_utf8_lossy(&stdout_buf);
                assert!(output.contains("cargo:rustc-env=VERGEN_GIT_SHA=this is a bad date"));
                assert!(output.contains("cargo:warning=VERGEN_GIT_SHA overidden"));
                Ok(())
            }();
            assert!(result.is_ok());
        });
    }

    #[test]
    #[serial]
    fn git_dirty_override_works() {
        with_var("VERGEN_GIT_DIRTY", Some("this is a bad date"), || {
            let result = || -> Result<()> {
                let mut stdout_buf = vec![];
                let git2 = Git2Builder::all_git()?;
                let _failed = Emitter::default()
                    .add_instructions(&git2)?
                    .emit_to(&mut stdout_buf)?;
                let output = String::from_utf8_lossy(&stdout_buf);
                assert!(output.contains("cargo:rustc-env=VERGEN_GIT_DIRTY=this is a bad date"));
                assert!(output.contains("cargo:warning=VERGEN_GIT_DIRTY overidden"));
                Ok(())
            }();
            assert!(result.is_ok());
        });
    }

    // #[cfg(feature = "gitcl")]
    // #[test]
    // #[serial_test::serial]
    // fn git_cmd_override_works() -> Result<()> {
    //     let mut stdout_buf = vec![];
    //     let failed = EmitBuilder::builder()
    //         .all_git()
    //         .git_cmd(Some("git -v"))
    //         .emit_to(&mut stdout_buf)?;
    //     let output = String::from_utf8_lossy(&stdout_buf);
    //     assert!(!failed);
    //     assert!(repo_exists().is_ok());
    //     assert!(GIT_REGEX_INST.is_match(&output));
    //     Ok(())
    // }
}
