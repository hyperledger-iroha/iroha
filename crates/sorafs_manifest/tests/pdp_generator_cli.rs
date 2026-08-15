//! Isolated-output and filesystem-safety tests for the PDP fixture generator.
use assert_cmd::cargo::cargo_bin_cmd;
use sorafs_manifest::{HashAlgorithmV1, PdpCommitmentV1};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
};
use tempfile::tempdir;
fn run_generator(current_dir: &Path, arguments: &[String]) -> Output {
    cargo_bin_cmd!("generate_pdp_fixtures")
        .current_dir(current_dir)
        .args(arguments)
        .output()
        .expect("run deterministic PDP fixture generator")
}
fn output_arguments(output_dir: &Path) -> Vec<String> {
    vec![
        "--output-dir".to_owned(),
        output_dir
            .to_str()
            .expect("temporary output path must be UTF-8")
            .to_owned(),
    ]
}
fn create_output_dir(root: &Path, name: &str) -> PathBuf {
    let output_dir = root.join(name);
    fs::create_dir_all(output_dir.join("negative")).expect("create isolated PDP output directory");
    output_dir
}
fn canonical_temp_root(path: &Path) -> PathBuf {
    fs::canonicalize(path).expect("canonicalize temporary generator root")
}
fn assert_failed_with(output: &Output, expected: &str) {
    assert!(
        !output.status.success(),
        "invalid generator invocation unexpectedly succeeded"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected),
        "generator failure did not contain `{expected}`:\n{stderr}"
    );
}
#[test]
fn isolated_output_generates_a_canonical_commitment_without_default_writes() {
    let root = tempdir().expect("create isolated generator root");
    let root_path = canonical_temp_root(root.path());
    let output_dir = create_output_dir(&root_path, "isolated-pdp");
    let output = run_generator(&root_path, &output_arguments(&output_dir));
    assert!(
        output.status.success(),
        "isolated generation failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes =
        fs::read(output_dir.join("commitment_v1.to")).expect("read isolated commitment fixture");
    let commitment: PdpCommitmentV1 =
        norito::decode_from_bytes(&bytes).expect("decode isolated commitment fixture");
    assert_eq!(commitment.hash_algorithm, HashAlgorithmV1::Blake3_256);
    assert!(
        !root_path.join("fixtures").exists(),
        "isolated generation must not write the default fixture tree"
    );
}
#[test]
fn output_dir_rejects_missing_duplicate_joined_and_ambiguous_values() {
    let root = tempdir().expect("create CLI rejection root");
    let root_path = canonical_temp_root(root.path());
    let output_dir = create_output_dir(&root_path, "valid-pdp");
    let output_text = output_dir
        .to_str()
        .expect("temporary output path must be UTF-8")
        .to_owned();
    let cases = [
        (
            vec!["--output-dir".to_owned()],
            "`--output-dir` requires a separate path argument",
        ),
        (
            vec![
                "--output-dir".to_owned(),
                output_text.clone(),
                "--output-dir".to_owned(),
                output_text.clone(),
            ],
            "`--output-dir` may be specified only once",
        ),
        (
            vec![format!("--output-dir={output_text}")],
            "unrecognized argument",
        ),
        (vec!["--unknown".to_owned()], "unrecognized argument"),
        (
            vec!["--output-dir".to_owned(), "--write".to_owned()],
            "`--output-dir` path must not be ambiguous with an option",
        ),
        (
            vec!["--output-dir".to_owned(), ".".to_owned()],
            "`--output-dir` path must not contain `.` components",
        ),
        (
            vec!["--output-dir".to_owned(), "../pdp".to_owned()],
            "`--output-dir` path must not contain `..` components",
        ),
    ];
    for (arguments, expected) in cases {
        assert_failed_with(&run_generator(&root_path, &arguments), expected);
    }
}
#[test]
fn output_dir_requires_an_existing_complete_directory() {
    let root = tempdir().expect("create incomplete output root");
    let root_path = canonical_temp_root(root.path());
    let missing = root_path.join("missing-pdp");
    assert_failed_with(
        &run_generator(&root_path, &output_arguments(&missing)),
        "failed to inspect PDP fixture output directory ancestry",
    );
    let incomplete = root_path.join("incomplete-pdp");
    fs::create_dir(&incomplete).expect("create incomplete PDP output directory");
    assert_failed_with(
        &run_generator(&root_path, &output_arguments(&incomplete)),
        "failed to inspect negative PDP fixture output directory ancestry",
    );
}
#[cfg(unix)]
#[test]
fn output_dir_rejects_symlinked_roots_and_ancestry() {
    use std::os::unix::fs::symlink;
    let root = tempdir().expect("create symlink rejection root");
    let root_path = canonical_temp_root(root.path());
    let real_output = create_output_dir(&root_path, "real-pdp");
    let output_link = root_path.join("pdp-link");
    symlink(&real_output, &output_link).expect("create output-root symlink");
    assert_failed_with(
        &run_generator(&root_path, &output_arguments(&output_link)),
        "ancestry must not contain a symbolic link",
    );
    let real_parent = root_path.join("real-parent");
    let nested_output = create_output_dir(&real_parent, "nested-pdp");
    let parent_link = root_path.join("parent-link");
    symlink(&real_parent, &parent_link).expect("create output-ancestor symlink");
    let linked_nested = parent_link.join(
        nested_output
            .file_name()
            .expect("nested output must have a name"),
    );
    assert_failed_with(
        &run_generator(&root_path, &output_arguments(&linked_nested)),
        "ancestry must not contain a symbolic link",
    );
}
#[cfg(unix)]
#[test]
fn output_dir_rejects_multiply_linked_fixture_targets() {
    let root = tempdir().expect("create hard-link rejection root");
    let root_path = canonical_temp_root(root.path());
    let output_dir = create_output_dir(&root_path, "hardlink-pdp");
    let external = root_path.join("external-commitment.to");
    fs::write(&external, b"external sentinel").expect("write external hard-link target");
    fs::hard_link(&external, output_dir.join("commitment_v1.to"))
        .expect("create multiply linked fixture target");
    let output = run_generator(&root_path, &output_arguments(&output_dir));
    assert_failed_with(&output, "must have exactly one hard link");
    assert_eq!(
        fs::read(&external).expect("read external hard-link target"),
        b"external sentinel",
        "failed generation must not alter an external hard-link alias"
    );
}
