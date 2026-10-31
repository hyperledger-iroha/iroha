//! Genuine native program for explicit-stream, quoting and creation-time Job controls.
#![cfg(windows)]

use std::{
    ffi::OsStr,
    io::{self, Read as _, Write as _},
    os::windows::ffi::OsStrExt as _,
    process::{Command, Stdio},
};

fn units(value: &OsStr) -> String {
    value
        .encode_wide()
        .map(|unit| format!("{unit:04x}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[repr(C)]
struct FileId {
    volume: u64,
    id: [u8; 16],
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetFileInformationByHandleEx(
        handle: *mut std::ffi::c_void,
        class: i32,
        output: *mut std::ffi::c_void,
        length: u32,
    ) -> i32;
}

fn main() -> io::Result<()> {
    let mode = std::env::args().nth(1).unwrap();
    match mode.as_str() {
        "inspect" => {
            for argument in std::env::args_os().skip(2) {
                println!("ARG:{}", units(&argument));
            }
            println!(
                "ENV:{}",
                units(&std::env::var_os("IROHA_OWNER_VALUE").unwrap())
            );
            println!(
                "ABSENT:{}",
                std::env::var_os("IROHA_OWNER_REMOVED").is_none()
            );
            println!("CWD:{}", units(std::env::current_dir()?.as_os_str()));
        }
        "echo" => {
            let mut bytes = Vec::new();
            io::stdin().read_to_end(&mut bytes)?;
            io::stdout().write_all(&bytes)?;
            io::stderr().write_all(&bytes)?;
        }
        "leaf" => {
            println!("leaf-ready");
            io::stdout().flush()?;
            let mut bytes = Vec::new();
            io::stdin().read_to_end(&mut bytes)?;
            if !bytes.is_empty() {
                return Err(io::Error::other("leaf accepts EOF only"));
            }
            println!("leaf-eof");
        }
        "tree-exit" | "tree-wait" => {
            // The surrounding test created this process inside its original Job.
            // Descendants inherit that Job at creation without a PID lookup.
            let child = Command::new(std::env::current_exe()?)
                .arg("leaf")
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?;
            if mode == "tree-wait" {
                let mut original = child;
                if original.wait().is_err() {
                    std::mem::forget(original);
                    return Err(io::Error::other("original leaf wait failed"));
                }
            } else {
                // Intentionally exit while the retained outer Job still owns the live descendant.
                drop(child);
            }
        }
        "unrelated-handle" => {
            let raw: usize = std::env::args().nth(2).unwrap().parse().unwrap();
            let expected = std::env::args().nth(3).unwrap();
            let mut identity = FileId {
                volume: 0,
                id: [0; 16],
            };
            // SAFETY: the native API validates this numeric slot. It is only a
            // negative inherited-handle observation, never process/file authority.
            let observed = unsafe {
                GetFileInformationByHandleEx(
                    raw as *mut std::ffi::c_void,
                    18,
                    (&raw mut identity).cast(),
                    size_of::<FileId>() as u32,
                )
            };
            if observed != 0 {
                let actual = format!("{}:{:?}", identity.volume, identity.id);
                if actual == expected {
                    return Err(io::Error::other("unrelated inheritable original leaked"));
                }
            }
            println!("unrelated-excluded");
        }
        "exit" => {}
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown native probe mode",
            ));
        }
    }
    Ok(())
}
