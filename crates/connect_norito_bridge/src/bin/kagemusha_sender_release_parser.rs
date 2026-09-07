//! Project a bounded local canonical sender Release command without granting native authority.

use std::{
    env,
    ffi::OsString,
    io::{Read as _, Write as _},
    path::PathBuf,
    process::ExitCode,
};

use connect_norito_bridge::{
    KAGEMUSHA_SENDER_RELEASE_COMMAND_MAX_BYTES_V1, kagemusha_sender_release_command_projection_v1,
};

fn parse_args(mut args: impl Iterator<Item = OsString>) -> Result<(PathBuf, [u8; 32]), String> {
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--command-file")) {
        return Err("usage: kagemusha_sender_release_parser --command-file COMMAND.norito --operation-id HEX64".to_owned());
    }
    let path = PathBuf::from(args.next().ok_or("missing command file")?);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--operation-id")) {
        return Err("missing --operation-id".to_owned());
    }
    let operation = args.next().ok_or("missing operation ID")?;
    let operation = operation.to_str().ok_or("operation ID must be ASCII")?;
    if args.next().is_some()
        || operation.len() != 64
        || !operation
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("expected one lowercase nonzero HEX64 operation ID".to_owned());
    }
    let operation_id: [u8; 32] = hex::decode(operation)
        .map_err(|_| "invalid operation ID")?
        .try_into()
        .map_err(|_| "invalid operation ID width")?;
    if operation_id == [0; 32] {
        return Err("operation ID is zero".to_owned());
    }
    Ok((path, operation_id))
}

fn read_projection(path: PathBuf, operation_id: [u8; 32]) -> Result<Vec<u8>, String> {
    // Reject a named pipe before opening it; metadata on the opened descriptor below also
    // checks replacements, and the bounded read cannot grow beyond the protocol limit.
    if !std::fs::metadata(&path)
        .map_err(|_| "cannot inspect local command path")?
        .is_file()
    {
        return Err("command must be a regular file".to_owned());
    }
    #[cfg(unix)]
    let file = {
        use std::os::unix::fs::OpenOptionsExt as _;
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| "cannot open local command file")?
    };
    #[cfg(not(unix))]
    let file = std::fs::File::open(path).map_err(|_| "cannot open local command file")?;
    let metadata = file.metadata().map_err(|_| "cannot inspect command file")?;
    if !metadata.is_file()
        || metadata.len() == 0
        || metadata.len() > KAGEMUSHA_SENDER_RELEASE_COMMAND_MAX_BYTES_V1 as u64
    {
        return Err("command must be a nonempty bounded regular file".to_owned());
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take((KAGEMUSHA_SENDER_RELEASE_COMMAND_MAX_BYTES_V1 + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read command file")?;
    kagemusha_sender_release_command_projection_v1(operation_id, &bytes)
}

fn run() -> Result<(), String> {
    let (path, operation_id) = parse_args(env::args_os().skip(1))?;
    let projection = read_projection(path, operation_id)?;
    std::io::stdout()
        .lock()
        .write_all(&projection)
        .map_err(|_| "cannot write structural projection".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_exact_named_local_input_and_nonzero_lowercase_selector() {
        let valid = hex::encode([7; 32]);
        let args = ["--command-file", "command.norito", "--operation-id", &valid];
        let (path, operation) = parse_args(args.iter().copied().map(OsString::from)).unwrap();
        assert_eq!(path, PathBuf::from("command.norito"));
        assert_eq!(operation, [7; 32]);
        for bad in [
            "00".repeat(32),
            "AB".repeat(32),
            "01".repeat(31),
            "zz".repeat(32),
        ] {
            assert!(
                parse_args(
                    ["--command-file", "command.norito", "--operation-id", &bad]
                        .iter()
                        .copied()
                        .map(OsString::from)
                )
                .is_err()
            );
        }
        assert!(parse_args(args[..3].iter().copied().map(OsString::from)).is_err());
        assert!(
            parse_args(
                args.iter()
                    .copied()
                    .map(OsString::from)
                    .chain([OsString::from("extra")])
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_nonregular_empty_oversized_and_malformed_files() {
        let directory = tempfile::tempdir().unwrap();
        assert!(read_projection(directory.path().to_path_buf(), [7; 32]).is_err());
        let path = directory.path().join("command.norito");
        assert!(read_projection(path.clone(), [7; 32]).is_err());
        for bytes in [
            Vec::new(),
            vec![0; KAGEMUSHA_SENDER_RELEASE_COMMAND_MAX_BYTES_V1 + 1],
            vec![1; 64],
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(read_projection(path.clone(), [7; 32]).is_err());
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
