//! One bounded private seed handoff for the fixed scaling generator.

#[cfg(unix)]
use std::io::Read as _;

#[cfg(unix)]
use color_eyre::eyre::ensure;
use color_eyre::eyre::{Result, eyre};
use zeroize::Zeroizing;

/// Consume exactly one ready inherited pipe before generating any private or public artifact.
#[cfg(unix)]
pub(in crate::localnet) fn read_development_seed(fd: i32) -> Result<Zeroizing<String>> {
    ensure!(
        (3..=65535).contains(&fd),
        "fixed scaling seed input is invalid"
    );
    let mut input = crate::secure_fs::take_seed_pipe(fd)
        .map_err(|_| eyre!("fixed scaling seed input is invalid"))?;
    let metadata =
        rustix::fs::fstat(&input).map_err(|_| eyre!("fixed scaling seed input is invalid"))?;
    let flags = rustix::fs::fcntl_getfl(&input)
        .map_err(|_| eyre!("fixed scaling seed input is invalid"))?;
    ensure!(
        metadata.st_uid == rustix::process::geteuid().as_raw()
            && flags.contains(rustix::fs::OFlags::NONBLOCK),
        "fixed scaling seed input is invalid"
    );
    let mut scratch = Zeroizing::new([0_u8; 65]);
    let length = input
        .read(scratch.as_mut())
        .map_err(|_| eyre!("fixed scaling seed input is invalid"))?;
    ensure!(
        length == 64
            && scratch[..64]
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
            && scratch[..64].iter().any(|byte| *byte != b'0'),
        "fixed scaling seed input is invalid"
    );
    let mut tail = Zeroizing::new([0_u8; 1]);
    ensure!(
        input
            .read(tail.as_mut())
            .map_err(|_| eyre!("fixed scaling seed input is invalid"))?
            == 0,
        "fixed scaling seed input is invalid"
    );
    drop(input);
    let seed = std::str::from_utf8(&scratch[..64])
        .map_err(|_| eyre!("fixed scaling seed input is invalid"))?;
    Ok(Zeroizing::new(seed.to_owned()))
}

/// Reject unsupported descriptor transport before generating any artifacts.
#[cfg(not(unix))]
pub(in crate::localnet) fn read_development_seed(_fd: i32) -> Result<Zeroizing<String>> {
    Err(eyre!("fixed scaling seed input is invalid"))
}

#[cfg(all(test, not(unix)))]
#[test]
fn fixed_seed_transport_is_rejected_without_unix_descriptor_support() {
    assert_eq!(
        read_development_seed(3).unwrap_err().to_string(),
        "fixed scaling seed input is invalid"
    );
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        fs::File,
        io::Write as _,
        os::fd::{FromRawFd as _, IntoRawFd as _},
    };

    #[allow(
        unsafe_code,
        reason = "test-only anonymous pipe construction transfers both owned ends"
    )]
    fn pipe() -> (File, File) {
        let mut descriptors = [-1; 2];
        // SAFETY: the array has space for both descriptors; success transfers both valid FDs.
        assert_eq!(unsafe { libc::pipe(descriptors.as_mut_ptr()) }, 0);
        // SAFETY: each freshly created descriptor is uniquely owned exactly once.
        unsafe {
            (
                File::from_raw_fd(descriptors[0]),
                File::from_raw_fd(descriptors[1]),
            )
        }
    }

    fn ready_pipe(raw: &[u8], blocking: bool, eof: bool) -> (File, Option<File>) {
        let (read, mut write) = pipe();
        let flags = rustix::fs::fcntl_getfl(&read).unwrap();
        if !blocking {
            rustix::fs::fcntl_setfl(&read, flags | rustix::fs::OFlags::NONBLOCK).unwrap();
        }
        write.write_all(raw).unwrap();
        (
            read,
            if eof {
                drop(write);
                None
            } else {
                Some(write)
            },
        )
    }

    #[test]
    fn fixed_seed_reader_consumes_only_exact_lowercase_nonzero_ready_payload() {
        let expected = "ab".repeat(32);
        let (read, writer) = ready_pipe(expected.as_bytes(), false, true);
        assert!(writer.is_none());
        assert_eq!(
            &*read_development_seed(read.into_raw_fd()).unwrap(),
            &expected
        );
        for raw in [
            vec![],
            vec![b'a'; 63],
            vec![b'a'; 65],
            vec![b'A'; 64],
            vec![b'z'; 64],
            vec![b'0'; 64],
            [vec![b'a'; 64], vec![b'\n']].concat(),
        ] {
            let (read, _) = ready_pipe(&raw, false, true);
            let error = read_development_seed(read.into_raw_fd()).unwrap_err();
            assert_eq!(error.to_string(), "fixed scaling seed input is invalid");
        }
    }

    #[test]
    fn fixed_seed_reader_never_waits_for_missing_bytes_or_eof() {
        for raw in [vec![], vec![b'a'; 64]] {
            let (read, _writer) = ready_pipe(&raw, false, false);
            assert!(read_development_seed(read.into_raw_fd()).is_err());
        }
        let (read, _writer) = ready_pipe(&[b'a'; 64], true, true);
        assert!(read_development_seed(read.into_raw_fd()).is_err());
    }

    #[test]
    fn fixed_seed_reader_rejects_wrong_descriptor_shapes_without_seed_files() {
        for fd in [-1, 0, 1, 2, 65536] {
            assert!(read_development_seed(fd).is_err());
        }
        let empty = tempfile::tempfile().unwrap();
        assert!(read_development_seed(empty.into_raw_fd()).is_err());
        let (_read, write) = pipe();
        assert!(read_development_seed(write.into_raw_fd()).is_err());
    }
}
