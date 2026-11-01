//! Actual native process controls for the common explicit command facade.

use super::*;
use std::{
    fs,
    io::{BufRead as _, Write as _},
    path::PathBuf,
    sync::OnceLock,
};

fn image() -> &'static PathBuf {
    static IMAGE: OnceLock<PathBuf> = OnceLock::new();
    IMAGE.get_or_init(|| {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("probe.rs");
        fs::write(&source, r#"//! Actual command facade fixture.
use std::io::{self, Read as _, Write as _};
fn main() -> io::Result<()> {
    match std::env::args().nth(1).unwrap().as_str() {
        "inspect" => { println!("{:?}", std::env::args().skip(2).collect::<Vec<_>>()); println!("{}", std::env::var("IROHA_COMMON_VALUE").unwrap()); }
        "cwd" => { io::stdout().write_all(&std::fs::read("marker")?)?; }
        "mark-cwd" => { std::fs::write(std::env::args().nth(2).unwrap(), b"executed")?; io::stdout().write_all(&std::fs::read("marker")?)?; }
        "echo" => { let mut bytes=Vec::new(); io::stdin().read_to_end(&mut bytes)?; io::stdout().write_all(&bytes)?; io::stderr().write_all(&bytes)?; }
        "wait" => { println!("ready"); io::stdout().flush()?; let mut bytes=Vec::new(); io::stdin().read_to_end(&mut bytes)?; io::stdout().write_all(&bytes)?; println!("eof"); }
        "fd" => { let bytes=std::fs::read("/proc/self/fd/200")?; io::stdout().write_all(&bytes)?; }
        "exit" => std::process::exit(19),
        _ => return Err(io::Error::other("invalid mode")),
    } Ok(())
}
"#).unwrap();
        let path = directory.path().join(if cfg!(windows) { "native common.exe" } else { "native-common" });
        let compiled = std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
            .args(["--edition=2024", "-C", "debuginfo=0"]).arg(&source).arg("-o").arg(&path).output().unwrap();
        assert!(compiled.status.success(), "{}", String::from_utf8_lossy(&compiled.stderr));
        let _ = directory.keep(); path
    })
}

fn command(mode: &str) -> ProcessCommand {
    let mut command = ProcessCommand::new(image()).unwrap();
    command.arg(mode).kill_on_drop(true);
    command
}

#[test]
fn explicit_settings_and_observations_reach_the_original_image() {
    assert!(ProcessCommand::new("relative").is_err());
    let mut command = command("inspect");
    let arguments = ["", "with space", "quote\"and\\slash", "line\nfeed"];
    command
        .args(arguments)
        .env_clear()
        .envs([("IROHA_COMMON_VALUE", "old")])
        .env_remove("IROHA_COMMON_VALUE")
        .env("IROHA_COMMON_VALUE", "final");
    let directory = tempfile::tempdir().unwrap();
    command.current_dir(directory.path());
    assert_eq!(command.get_program(), image().as_os_str());
    assert_eq!(command.get_current_dir(), Some(directory.path()));
    assert_eq!(command.get_args().skip(1).collect::<Vec<_>>(), arguments);
    assert_eq!(
        command.get_envs().collect::<Vec<_>>(),
        vec![(OsStr::new("IROHA_COMMON_VALUE"), Some(OsStr::new("final")))]
    );
    let output = command.output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, format!("{arguments:?}\nfinal\n").as_bytes());
    assert!(output.stderr.is_empty());
}

#[test]
fn explicit_file_and_pipe_streams_preserve_binary_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("input");
    let output_path = directory.path().join("output");
    let bytes = (0_u8..=255).cycle().take(262_144).collect::<Vec<_>>();
    fs::write(&input_path, &bytes).unwrap();
    let mut command = command("echo");
    command
        .stdin(fs::File::open(input_path).unwrap())
        .stdout(fs::File::create(&output_path).unwrap())
        .stderr(ProcessStdio::piped());
    let output = command.spawn().unwrap().wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, bytes);
    assert_eq!(fs::read(output_path).unwrap(), bytes);
}

#[test]
fn wait_closes_owned_input_and_keeps_terminal_status() {
    let mut command = command("wait");
    command
        .stdin(ProcessStdio::piped())
        .stdout(ProcessStdio::piped());
    let mut child = command.spawn().unwrap();
    assert!(child.id() > 0);
    let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    assert!(child.try_wait().unwrap().is_none());
    child.stdin.as_mut().unwrap().write_all(b"input\0").unwrap();
    let status = child.wait().unwrap();
    assert!(status.success());
    assert_eq!(child.try_wait().unwrap(), Some(status));
    let mut bytes = Vec::new();
    output.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"input\0eof\n");
}

#[test]
fn drop_with_external_input_terminates_owned_original() {
    let mut command = command("wait");
    command
        .stdin(ProcessStdio::piped())
        .stdout(ProcessStdio::piped());
    let mut child = command.spawn().unwrap();
    let input = child.stdin.take().unwrap();
    let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    drop(child);
    line.clear();
    output.read_to_string(&mut line).unwrap();
    assert!(line.is_empty());
    drop(input);
}

#[test]
fn status_preserves_the_original_exit_code() {
    let mut command = command("exit");
    command
        .stdin(ProcessStdio::null())
        .stdout(ProcessStdio::inherit())
        .stderr(ProcessStdio::null());
    assert_eq!(command.status().unwrap().code(), Some(19));
}

struct ThreadWake(thread::Thread);

impl std::task::Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let waker = std::task::Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        if let std::task::Poll::Ready(result) = future.as_mut().poll(&mut context) {
            return result;
        }
        thread::park();
    }
}

#[test]
fn async_wait_requires_no_runtime_and_cancellation_preserves_the_original() {
    let mut command = command("wait");
    command
        .stdin(ProcessStdio::piped())
        .stdout(ProcessStdio::piped());
    let mut child = command.spawn().unwrap();
    let input = child.stdin.take().unwrap();
    let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    let waker = std::task::Waker::from(Arc::new(ThreadWake(thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut waiting = Box::pin(child.wait_async());
    assert!(std::future::Future::poll(waiting.as_mut(), &mut context).is_pending());
    drop(waiting);
    assert!(child.try_wait().unwrap().is_none());
    drop(input);
    assert!(block_on(child.wait_async()).unwrap().success());
    line.clear();
    output.read_to_string(&mut line).unwrap();
    assert_eq!(line, "eof\n");
}

#[cfg(target_os = "linux")]
#[test]
fn descriptor_inputs_survive_original_drop_without_raw_command_escape() {
    use std::os::fd::AsFd as _;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input");
    fs::write(&path, b"retained descriptor\0").unwrap();
    let input = fs::File::open(path).unwrap();
    let mut command = command("fd");
    command
        .process_group(0)
        .inherit_descriptors(&[(input.as_fd(), 200)])
        .unwrap();
    drop(input);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"retained descriptor\0");
}

#[cfg(not(target_os = "fuchsia"))]
#[path = "current_directory_tests.rs"]
mod current_directory_tests;
