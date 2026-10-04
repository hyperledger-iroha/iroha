//! Native executable format admission shared by CLI packaging and runtime qualification.

use iroha_fs::RetainedFile;
use std::{
    env,
    io::{self, Read, Seek, SeekFrom},
};

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Admit one retained executable for this exact native operating system and architecture.
///
/// This checks executable format and custody, not authenticated build provenance.
///
/// # Errors
/// Rejects nonexecutable, cross-platform, cross-architecture, nonregular or changed inputs.
pub fn admit_native_program(file: &mut RetainedFile) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if file.file().metadata()?.permissions().mode() & 0o100 == 0 {
            return Err(invalid("CLI program is not executable"));
        }
    }
    file.file_mut().seek(SeekFrom::Start(0))?;
    let mut header = [0_u8; 64];
    file.file_mut().read_exact(&mut header)?;
    let native = match (env::consts::OS, env::consts::ARCH) {
        ("macos", arch) => {
            let cpu = match arch {
                "aarch64" => 0x0100_000c,
                "x86_64" => 0x0100_0007,
                _ => return Err(invalid("unsupported native CLI architecture")),
            };
            header[..4] == [0xcf, 0xfa, 0xed, 0xfe]
                && u32::from_le_bytes(header[4..8].try_into().expect("fixed native header field"))
                    == cpu
                && u32::from_le_bytes(
                    header[12..16]
                        .try_into()
                        .expect("fixed native header field"),
                ) == 2
        }
        ("linux", arch) => {
            let cpu = match arch {
                "aarch64" => 183,
                "x86_64" => 62,
                _ => return Err(invalid("unsupported native CLI architecture")),
            };
            header[..7] == [0x7f, b'E', b'L', b'F', 2, 1, 1]
                && matches!(
                    u16::from_le_bytes(
                        header[16..18]
                            .try_into()
                            .expect("fixed native header field")
                    ),
                    2 | 3
                )
                && u16::from_le_bytes(
                    header[18..20]
                        .try_into()
                        .expect("fixed native header field"),
                ) == cpu
                && u64::from_le_bytes(
                    header[24..32]
                        .try_into()
                        .expect("fixed native header field"),
                ) != 0
        }
        ("windows", arch) => {
            let cpu = match arch {
                "aarch64" => 0xaa64,
                "x86_64" => 0x8664,
                _ => return Err(invalid("unsupported native CLI architecture")),
            };
            let offset = u32::from_le_bytes(
                header[60..64]
                    .try_into()
                    .expect("fixed native header field"),
            );
            if header[..2] != *b"MZ" || !(64..=1024 * 1024).contains(&offset) {
                return Err(invalid("invalid native PE executable"));
            }
            file.file_mut().seek(SeekFrom::Start(u64::from(offset)))?;
            let mut pe = [0_u8; 26];
            file.file_mut().read_exact(&mut pe)?;
            let flags =
                u16::from_le_bytes(pe[22..24].try_into().expect("fixed native header field"));
            pe[..4] == *b"PE\0\0"
                && u16::from_le_bytes(pe[4..6].try_into().expect("fixed native header field"))
                    == cpu
                && flags & 2 != 0
                && flags & 0x2000 == 0
                && u16::from_le_bytes(pe[24..26].try_into().expect("fixed native header field"))
                    == 0x20b
        }
        _ => return Err(invalid("unsupported native CLI operating system")),
    };
    if !native {
        return Err(invalid(
            "CLI artifact is not an executable for this native host",
        ));
    }
    file.revalidate()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_program_admits_actual_current_harness_and_refuses_plain_text() {
        let mut original = RetainedFile::open_regular(std::env::current_exe().unwrap()).unwrap();
        admit_native_program(&mut original).unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("nonexecutable");
        std::fs::write(&path, b"ordinary text is not a native executable").unwrap();
        let mut text = RetainedFile::open_regular(&path).unwrap();
        assert!(admit_native_program(&mut text).is_err());
    }
}
