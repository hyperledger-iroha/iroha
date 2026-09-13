//! Process-stream adapter for the Cargo-style Musubi V1 command shell.
use crate::{command, output::ErrorCode};
use std::io::{self, Write as _};
/// Parse process arguments, execute one command, and write its routed output.
///
/// The returned value is the stable process exit status. The library never
/// terminates the process itself, which keeps parsing and output testable.
pub fn run() -> i32 {
    let invocation = command::invoke_with_progress(std::env::args_os(), &mut |message| {
        // Progress is presentation only: a closed terminal must not alter native recovery policy.
        let mut stderr = io::stderr().lock();
        let _ = writeln!(stderr, "{message}");
        let _ = stderr.flush();
    });
    let Ok(rendered) = invocation.output.render(invocation.format) else {
        return ErrorCode::Internal.exit_code();
    };
    let exit_code = rendered.exit_code();
    if rendered
        .write_to(&mut io::stdout().lock(), &mut io::stderr().lock())
        .is_err()
    {
        return ErrorCode::Io.exit_code();
    }
    exit_code
}
