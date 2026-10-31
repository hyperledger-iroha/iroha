//! Actual native images and original-handle controls for the process-owner draft.

use super::*;
use std::{
    ffi::{OsStr, OsString},
    fmt::Write as _,
    fs,
    io::{BufRead as _, Read as _, Write as _},
    os::windows::ffi::{OsStrExt as _, OsStringExt as _},
    path::PathBuf,
    process::Command as Compiler,
    sync::OnceLock,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Storage::FileSystem::{FILE_ID_INFO, FileIdInfo, GetFileInformationByHandleEx},
    System::JobObjects::IsProcessInJob,
};

fn probe() -> &'static PathBuf {
    static IMAGE: OnceLock<PathBuf> = OnceLock::new();
    IMAGE.get_or_init(|| {
        let directory = tempfile::Builder::new()
            .prefix("iroha-original-process-")
            .tempdir()
            .unwrap();
        let source = directory.path().join("program.rs");
        fs::write(&source, include_bytes!("../../process_owner_probe.rs")).unwrap();
        let image = directory.path().join("program with space.exe");
        let output = Compiler::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2024", "-C", "debuginfo=0"])
            .arg(source)
            .arg("-o")
            .arg(&image)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        directory.keep().join("program with space.exe")
    })
}

fn command(mode: &str) -> Command {
    let mut command = Command::new(probe()).unwrap();
    command
        .arg(mode)
        .stdin(Stream::Piped)
        .stdout(Stream::Piped)
        .stderr(Stream::Piped)
        .kill_on_drop(true);
    command
}

fn collect(mut child: Child, input: &[u8]) -> (ExitStatus, Vec<u8>, Vec<u8>) {
    let mut output = child.stdout.take().unwrap();
    let mut error = child.stderr.take().unwrap();
    let output = thread::spawn(move || {
        let mut bytes = Vec::new();
        output.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let error = thread::spawn(move || {
        let mut bytes = Vec::new();
        error.read_to_end(&mut bytes).unwrap();
        bytes
    });
    if let Some(mut writer) = child.stdin.take() {
        writer.write_all(input).unwrap();
    }
    let status = child.wait().unwrap();
    (status, output.join().unwrap(), error.join().unwrap())
}

fn units(value: &OsStr) -> String {
    value
        .encode_wide()
        .map(|unit| format!("{unit:04x}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn creation_installs_job_before_root_and_descendant_can_run() {
    let mut child = command("tree-wait").spawn().unwrap();
    let mut reader = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line, "leaf-ready\n");
    let original = child.originals.as_ref().unwrap();
    let mut member = 0;
    // SAFETY: compare only the retained original process and its original unnamed Job.
    assert_ne!(
        unsafe {
            IsProcessInJob(
                original.process.as_raw_handle(),
                original.job.raw(),
                &raw mut member,
            )
        },
        0
    );
    assert_ne!(member, 0);
    assert_eq!(original.job.active_processes().unwrap(), 2);
    assert!(child.try_wait().unwrap().is_none());
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());
    assert_eq!(
        child
            .originals
            .as_ref()
            .unwrap()
            .job
            .active_processes()
            .unwrap(),
        0
    );
    line.clear();
    reader.read_to_string(&mut line).unwrap();
    assert!(line.is_empty());
}

#[test]
fn root_exit_does_not_report_whole_job_exit_while_descendant_holds_input() {
    let mut child = command("tree-exit").spawn().unwrap();
    let mut reader = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line, "leaf-ready\n");
    assert!(
        child
            .originals
            .as_mut()
            .unwrap()
            .root_status(INFINITE)
            .unwrap()
            .unwrap()
            .success()
    );
    assert_eq!(
        child
            .originals
            .as_ref()
            .unwrap()
            .job
            .active_processes()
            .unwrap(),
        1
    );
    assert!(
        child.try_wait().unwrap().is_none(),
        "root-only exit must not release whole-Job custody"
    );
    drop(child.stdin.take());
    line.clear();
    reader.read_to_string(&mut line).unwrap();
    assert_eq!(line, "leaf-eof\n");
    assert!(child.wait().unwrap().success());
    assert!(child.try_wait().unwrap().unwrap().success());
}

#[test]
fn quoting_environment_and_working_directory_reach_actual_native_program_exactly() {
    let directory = tempfile::Builder::new()
        .prefix("iroha owner cwd ")
        .tempdir()
        .unwrap();
    let arguments = [
        OsString::new(),
        OsString::from("space tab\t newline\n"),
        OsString::from("trailing\\\\"),
        OsString::from("before\\\"quote\"after"),
        OsString::from("日本語 🦀"),
        OsString::from_wide(&[0xd800, 0x61, 0xdc00]),
    ];
    let mut selected = command("inspect");
    selected
        .env_clear()
        .env("IROHA_OWNER_VALUE", "first")
        .env("iroha_owner_value", "last value\\\"")
        .env("IROHA_OWNER_REMOVED", "remove")
        .env_remove("iroha_owner_removed")
        .current_dir(directory.path());
    for argument in &arguments {
        selected.arg(argument);
    }
    let (status, output, error) = collect(selected.spawn().unwrap(), b"");
    assert!(status.success(), "{}", String::from_utf8_lossy(&error));
    let mut expected = String::new();
    for argument in &arguments {
        writeln!(&mut expected, "ARG:{}", units(argument)).unwrap();
    }
    writeln!(
        &mut expected,
        "ENV:{}\nABSENT:true\nCWD:{}",
        units(OsStr::new("last value\\\"")),
        units(directory.path().as_os_str())
    )
    .unwrap();
    assert_eq!(output, expected.as_bytes());
}

#[test]
fn explicit_pipes_preserve_binary_bytes_and_concurrent_output() {
    let input: Vec<u8> = (0_u8..=255).cycle().take(262_144).collect();
    let (status, output, error) = collect(command("echo").spawn().unwrap(), &input);
    assert!(status.success());
    assert_eq!(output, input);
    assert_eq!(error, input);
}

#[test]
fn explicit_owned_file_stream_keeps_the_selected_original() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("output");
    let file = File::create(&output).unwrap();
    let mut selected = command("echo");
    selected.stdout(file).stderr(Stream::Null);
    let mut child = selected.spawn().unwrap();
    assert!(child.stdout.is_none());
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"original\r\n\x1a\0")
        .unwrap();
    assert!(child.wait().unwrap().success());
    assert_eq!(fs::read(output).unwrap(), b"original\r\n\x1a\0");
}

#[test]
fn tokio_from_std_uses_actual_overlapped_originals_for_pending_binary_io() {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut child = command("echo").spawn().unwrap();
    let input: Vec<u8> = (0_u8..=255).cycle().take(262_144).collect();
    runtime.block_on(async {
        let mut stdin = tokio::process::ChildStdin::from_std(child.stdin.take().unwrap()).unwrap();
        let mut stdout =
            tokio::process::ChildStdout::from_std(child.stdout.take().unwrap()).unwrap();
        let mut stderr =
            tokio::process::ChildStderr::from_std(child.stderr.take().unwrap()).unwrap();
        let roundtrip = async {
            let write = async {
                stdin.write_all(&input).await.unwrap();
                stdin.shutdown().await.unwrap();
                drop(stdin);
            };
            let read_output = async {
                let mut bytes = Vec::new();
                stdout.read_to_end(&mut bytes).await.unwrap();
                bytes
            };
            let read_error = async {
                let mut bytes = Vec::new();
                stderr.read_to_end(&mut bytes).await.unwrap();
                bytes
            };
            let ((), output, error) = tokio::join!(write, read_output, read_error);
            assert_eq!(output, input);
            assert_eq!(error, input);
        };
        tokio::time::timeout(Duration::from_secs(20), roundtrip)
            .await
            .unwrap();
    });
    assert!(child.wait().unwrap().success());
}

#[test]
fn cancelled_wait_and_destroyed_runtime_leave_original_job_and_descendant_owned() {
    let mut child = command("tree-exit").spawn().unwrap();
    let input = child.stdin.take().unwrap();
    let mut reader = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line, "leaf-ready\n");
    assert!(
        child
            .originals
            .as_mut()
            .unwrap()
            .root_status(INFINITE)
            .unwrap()
            .unwrap()
            .success()
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let wait = async {
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        };
        assert!(
            tokio::time::timeout(Duration::from_millis(20), wait)
                .await
                .is_err()
        );
    });
    drop(runtime);
    assert_eq!(
        child
            .originals
            .as_ref()
            .unwrap()
            .job
            .active_processes()
            .unwrap(),
        1
    );
    assert!(child.try_wait().unwrap().is_none());
    drop(input);
    line.clear();
    reader.read_to_string(&mut line).unwrap();
    assert_eq!(line, "leaf-eof\n");
    assert!(child.wait().unwrap().success());
}

#[test]
fn inherited_standard_streams_and_empty_native_exit_are_supported() {
    let mut selected = Command::new(probe()).unwrap();
    selected.arg("exit");
    let mut child = selected.spawn().unwrap();
    assert!(child.stdin.is_none() && child.stdout.is_none() && child.stderr.is_none());
    assert!(child.wait().unwrap().success());
}

#[test]
fn own_handle_list_excludes_an_unrelated_inheritable_file_original() {
    let directory = tempfile::tempdir().unwrap();
    let file = File::create(directory.path().join("unrelated")).unwrap();
    let inherited = streams::duplicate(file.as_raw_handle(), true).unwrap();
    // SAFETY: exact native identity output is filled from the retained unrelated original.
    let mut info: FILE_ID_INFO = unsafe { std::mem::zeroed() };
    assert_ne!(
        unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileIdInfo,
                (&raw mut info).cast(),
                u32::try_from(size_of::<FILE_ID_INFO>()).unwrap(),
            )
        },
        0
    );
    let expected = format!("{}:{:?}", info.VolumeSerialNumber, info.FileId.Identifier);
    let mut selected = command("unrelated-handle");
    selected
        .arg((inherited.as_raw_handle() as usize).to_string())
        .arg(expected);
    let (status, output, error) = collect(selected.spawn().unwrap(), b"");
    assert!(status.success(), "{}", String::from_utf8_lossy(&error));
    assert_eq!(output, b"unrelated-excluded\n");
}

#[test]
fn invalid_settings_refuse_before_a_process_is_created() {
    assert!(Command::new("relative.exe").is_err());
    let mut nul_argument = command("exit");
    nul_argument.arg(OsString::from_wide(&[u16::from(b'a'), 0, u16::from(b'b')]));
    assert!(
        matches!(nul_argument.spawn(), Err(error) if error.kind() == io::ErrorKind::InvalidInput)
    );
    let mut invalid_environment = command("exit");
    invalid_environment.env("name=invalid", "value");
    assert!(
        matches!(invalid_environment.spawn(), Err(error) if error.kind() == io::ErrorKind::InvalidInput)
    );
    let mut invalid_cwd = command("exit");
    invalid_cwd.current_dir("relative");
    assert!(
        matches!(invalid_cwd.spawn(), Err(error) if error.kind() == io::ErrorKind::InvalidInput)
    );
    let directory = tempfile::tempdir().unwrap();
    assert!(
        Command::new(directory.path().join("missing.exe"))
            .unwrap()
            .spawn()
            .is_err()
    );
}

#[test]
fn job_and_process_originals_cannot_be_inherited_as_streams() {
    let job = Job::new().unwrap();
    let mut selected = command("exit");
    selected.stdout(Stream::Owned(job.handle().try_clone_to_owned().unwrap()));
    assert!(matches!(selected.spawn(), Err(error) if error.kind() == io::ErrorKind::InvalidInput));
    assert_eq!(job.active_processes().unwrap(), 0);

    let mut original = command("leaf").spawn().unwrap();
    let mut reader = std::io::BufReader::new(original.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line, "leaf-ready\n");
    let mut selected = command("exit");
    selected.stderr(Stream::Owned(
        original.as_handle().try_clone_to_owned().unwrap(),
    ));
    assert!(matches!(selected.spawn(), Err(error) if error.kind() == io::ErrorKind::InvalidInput));
    assert!(original.try_wait().unwrap().is_none());
    drop(original.stdin.take());
    assert!(original.wait().unwrap().success());
    line.clear();
    reader.read_to_string(&mut line).unwrap();
    assert_eq!(line, "leaf-eof\n");
}

#[test]
fn dropping_live_owner_terminates_only_its_original_job_and_observes_all_members() {
    let mut child = command("tree-wait").spawn().unwrap();
    // Keep the original input outside the dropping supervisor so its Drop cannot
    // give the leaf a graceful EOF before Job termination is exercised.
    let input = child.stdin.take().unwrap();
    let mut reader = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert_eq!(line, "leaf-ready\n");
    // Retain a diagnostic duplicate of our exact process original; no PID recovery.
    let process = streams::duplicate(child.as_raw_handle(), false).unwrap();
    drop(child);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        // SAFETY: observe only our duplicated original process handle.
        if unsafe { WaitForSingleObject(process.as_raw_handle(), 0) } == WAIT_OBJECT_0 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "original process did not terminate after owned Job drop"
        );
        thread::sleep(Duration::from_millis(10));
    }
    line.clear();
    reader.read_to_string(&mut line).unwrap();
    assert!(line.is_empty());
    drop(input);
}
