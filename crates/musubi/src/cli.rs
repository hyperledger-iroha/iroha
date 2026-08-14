//! Process-stream adapter for the Cargo-style Musubi V1 command shell.
use std::io;
use crate::{command, output::ErrorCode};
/// Parse process arguments, execute one command, and write its routed output.
///
/// The returned value is the stable process exit status. The library never
/// terminates the process itself, which keeps parsing and output testable.
pub fn run() -> i32 {
    let invocation = command::invoke(std::env::args_os());
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
