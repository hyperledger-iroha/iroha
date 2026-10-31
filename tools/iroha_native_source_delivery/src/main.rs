//! Persistent native custody for one authenticated Python source promotion.
//!
//! The Python owner authenticates source, wheel, manifest and receipt semantics. This
//! process retains the original objects across its checks and performs only bounded
//! creation and no-replace moves. EOF or any error retains diagnostic files without cleanup.

use std::{
    io::{self, BufRead, Read, Seek, Write},
    path::Path,
};

use iroha_fs::{
    MovableFile, MovedFile, OwnerDirectory, PendingPrivateFile, PrivateDirectory, SealedPrivateFile,
};
use norito::json::{self, Value};
use sha2::{Digest, Sha256};

const MAX_REQUEST: u64 = 1024 * 1024;
const MAX_CONTROL: usize = 64 * 1024;
const MAX_NATIVE: u64 = 256 * 1024 * 1024;
const MAX_CHUNK: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Begun,
    Intent,
    Writing,
    Sealed,
    BackedUp,
    Active,
    Receipted,
    Complete,
}

struct Session {
    source: OwnerDirectory,
    backup: OwnerDirectory,
    private_backup: PrivateDirectory,
    name: String,
    old: Option<MovableFile>,
    old_hash: Option<String>,
    preserved: Option<MovedFile>,
    writer: Option<PendingPrivateFile>,
    fresh: Option<MovableFile>,
    active: Option<MovedFile>,
    controls: Vec<SealedPrivateFile>,
    length: u64,
    written: u64,
    expected_hash: String,
    hash: Sha256,
    phase: Phase,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn object<'a>(value: &'a Value, names: &[&str]) -> io::Result<&'a json::Map> {
    let fields = value
        .as_object()
        .ok_or_else(|| invalid("expected request object"))?;
    if fields.len() != names.len() || names.iter().any(|name| !fields.contains_key(*name)) {
        return Err(invalid("request field inventory is not exact"));
    }
    Ok(fields)
}

fn text<'a>(fields: &'a json::Map, name: &str) -> io::Result<&'a str> {
    fields
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("expected text field"))
}

fn digest(value: &str) -> io::Result<String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("expected lowercase SHA-256"));
    }
    Ok(value.to_owned())
}

fn hash_reader(reader: &mut (impl Read + Seek)) -> io::Result<String> {
    reader.rewind()?;
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut chunk = vec![0u8; MAX_CHUNK];
    loop {
        let read = reader.read(&mut chunk)?;
        if read == 0 {
            break;
        }
        count += read as u64;
        if count > MAX_NATIVE {
            return Err(invalid("native file exceeds byte ceiling"));
        }
        hash.update(&chunk[..read]);
    }
    reader.rewind()?;
    Ok(hex::encode(hash.finalize()))
}

fn verify_control_bytes(file: &mut SealedPrivateFile, expected: &[u8]) -> io::Result<()> {
    file.rewind()?;
    let mut observed = Vec::new();
    file.by_ref()
        .take(MAX_CONTROL as u64 + 1)
        .read_to_end(&mut observed)?;
    if observed != expected {
        return Err(invalid(
            "retained control bytes differ from authored content",
        ));
    }
    file.revalidate()
}

impl Session {
    fn begin(value: &Value) -> io::Result<Self> {
        let fields = object(value, &["op", "source", "backup", "name", "old_sha256"])?;
        if text(fields, "op")? != "begin" {
            return Err(invalid("first request must begin custody"));
        }
        let source = OwnerDirectory::open(Path::new(text(fields, "source")?))?;
        let backup_path = Path::new(text(fields, "backup")?);
        let parent = OwnerDirectory::open(
            backup_path
                .parent()
                .ok_or_else(|| invalid("backup has no parent"))?,
        )?;
        let backup_name = backup_path
            .file_name()
            .ok_or_else(|| invalid("backup has no basename"))?;
        let private_backup = match parent.open_private_child_optional(backup_name)? {
            Some(directory) => directory,
            None => parent.create_private_child(backup_name)?,
        };
        let backup = OwnerDirectory::open(private_backup.path())?;
        if source.identity()? == backup.identity()? || !backup.entries(1)?.is_empty() {
            return Err(invalid(
                "backup must be a distinct empty retained directory",
            ));
        }
        let name = text(fields, "name")?.to_owned();
        let old_hash = match fields.get("old_sha256") {
            Some(Value::Null) => None,
            Some(Value::String(hash)) => Some(digest(hash)?),
            _ => return Err(invalid("invalid original hash")),
        };
        let old = if let Some(expected) = &old_hash {
            let mut file = source.open_movable_regular(&name)?;
            if hash_reader(&mut file)? != *expected {
                return Err(invalid("original native bytes changed"));
            }
            Some(file)
        } else {
            if source.open_regular_optional(&name)?.is_some() {
                return Err(invalid("unexpected original native file"));
            }
            None
        };
        let session = Self {
            source,
            backup,
            private_backup,
            name,
            old,
            old_hash,
            preserved: None,
            writer: None,
            fresh: None,
            active: None,
            controls: Vec::new(),
            length: 0,
            written: 0,
            expected_hash: String::new(),
            hash: Sha256::new(),
            phase: Phase::Begun,
        };
        session.check()?;
        Ok(session)
    }

    fn check(&self) -> io::Result<()> {
        self.source.revalidate()?;
        self.backup.revalidate()?;
        self.private_backup.revalidate()?;
        if let Some(file) = &self.old {
            file.revalidate()?;
        }
        if let Some(file) = &self.fresh {
            file.revalidate()?;
        }
        if let Some(file) = &self.preserved {
            file.revalidate()?;
        }
        if let Some(file) = &self.active {
            file.require_source_absent()?;
        }
        for file in &self.controls {
            file.revalidate()?;
        }
        Ok(())
    }

    fn control(&mut self, name: &str, contents: &str) -> io::Result<()> {
        if contents.is_empty() || contents.len() > MAX_CONTROL {
            return Err(invalid("control exceeds byte ceiling"));
        }
        let mut file = self.backup.create_retained_private(name, MAX_CONTROL)?;
        file.write_all(contents.as_bytes())?;
        let mut sealed = file.seal_read_only()?;
        verify_control_bytes(&mut sealed, contents.as_bytes())?;
        self.controls.push(sealed);
        self.backup.sync()?;
        self.check()
    }

    fn request(&mut self, value: &Value) -> io::Result<()> {
        self.check()?;
        let operation = value
            .get("op")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("missing operation"))?;
        match (self.phase, operation) {
            (Phase::Begun, "intent") => {
                let fields = object(value, &["op", "json"])?;
                self.control("intent.json", text(fields, "json")?)?;
                self.backup.sync_parent()?;
                self.phase = Phase::Intent;
            }
            (Phase::Intent, "stage") => {
                let fields = object(value, &["op", "name", "length", "sha256"])?;
                let length = fields
                    .get("length")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid("invalid native length"))?;
                if length == 0 || length > MAX_NATIVE {
                    return Err(invalid("native length exceeds bound"));
                }
                let name = text(fields, "name")?;
                if !name.starts_with(".fresh-native-") || name == self.name {
                    return Err(invalid("invalid private staging name"));
                }
                self.expected_hash = digest(text(fields, "sha256")?)?;
                let maximum = usize::try_from(length)
                    .map_err(|_| invalid("native length exceeds host bound"))?;
                self.writer = Some(self.backup.create_retained_private(name, maximum)?);
                self.length = length;
                self.phase = Phase::Writing;
            }
            (Phase::Writing, "chunk") => {
                let fields = object(value, &["op", "hex"])?;
                let encoded = text(fields, "hex")?;
                if encoded.is_empty() || encoded.len() > MAX_CHUNK * 2 {
                    return Err(invalid("invalid chunk bound"));
                }
                let bytes = hex::decode(encoded).map_err(|_| invalid("invalid chunk hex"))?;
                if self.written + bytes.len() as u64 > self.length {
                    return Err(invalid("native length exceeded"));
                }
                self.writer
                    .as_mut()
                    .ok_or_else(|| invalid("missing original writer"))?
                    .write_all(&bytes)?;
                self.hash.update(&bytes);
                self.written += bytes.len() as u64;
            }
            (Phase::Writing, "seal") => {
                object(value, &["op"])?;
                if self.written != self.length
                    || hex::encode(self.hash.clone().finalize()) != self.expected_hash
                {
                    return Err(invalid(
                        "fresh native bytes differ from authenticated member",
                    ));
                }
                let writer = self
                    .writer
                    .take()
                    .ok_or_else(|| invalid("missing original writer"))?;
                let mut fresh = writer.seal_executable()?;
                if hash_reader(&mut fresh)? != self.expected_hash {
                    return Err(invalid("fresh native readback differs"));
                }
                self.fresh = Some(fresh);
                self.backup.sync()?;
                self.phase = Phase::Sealed;
            }
            (Phase::Sealed, "backup") => {
                object(value, &["op"])?;
                if let Some(old) = self.old.take() {
                    let mut moved = old.move_no_replace(&self.backup, &self.name)?;
                    if Some(hash_reader(&mut moved)?) != self.old_hash {
                        return Err(invalid("preserved original bytes changed"));
                    }
                    self.preserved = Some(moved);
                }
                self.phase = Phase::BackedUp;
            }
            (Phase::BackedUp, "activate") => {
                object(value, &["op"])?;
                if let Some(file) = &self.preserved {
                    file.require_source_absent()?;
                }
                let fresh = self
                    .fresh
                    .take()
                    .ok_or_else(|| invalid("missing original fresh file"))?;
                let mut active = fresh.move_no_replace(&self.source, &self.name)?;
                if hash_reader(&mut active)? != self.expected_hash {
                    return Err(invalid("activated native bytes changed"));
                }
                self.active = Some(active);
                self.phase = Phase::Active;
            }
            (Phase::Active, "receipt") => {
                let fields = object(value, &["op", "json"])?;
                self.control("delivery.json", text(fields, "json")?)?;
                self.phase = Phase::Receipted;
            }
            (Phase::Receipted, "complete") => {
                object(value, &["op"])?;
                if let Some(file) = &mut self.preserved
                    && Some(hash_reader(file)?) != self.old_hash
                {
                    return Err(invalid("final original bytes changed"));
                }
                if hash_reader(
                    self.active
                        .as_mut()
                        .ok_or_else(|| invalid("missing active original"))?,
                )? != self.expected_hash
                {
                    return Err(invalid("final active bytes changed"));
                }
                self.phase = Phase::Complete;
            }
            _ => return Err(invalid("operation is not valid at this delivery boundary")),
        }
        self.check()
    }
}

fn run(mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut session = None;
    loop {
        let mut bytes = Vec::new();
        input
            .by_ref()
            .take(MAX_REQUEST + 1)
            .read_until(b'\n', &mut bytes)?;
        if bytes.is_empty() {
            return Err(invalid("delivery ended before its complete receipt"));
        }
        if bytes.len() as u64 > MAX_REQUEST || bytes.last() != Some(&b'\n') {
            return Err(invalid("request exceeds framing bound"));
        }
        let request: Value =
            json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
        if let Some(session) = &mut session {
            Session::request(session, &request)?;
        } else {
            session = Some(Session::begin(&request)?);
        }
        output.write_all(b"{}\n")?;
        output.flush()?;
        if session
            .as_ref()
            .is_some_and(|session| session.phase == Phase::Complete)
        {
            return Ok(());
        }
    }
}

fn main() -> std::process::ExitCode {
    if std::env::args_os().len() != 1 {
        eprintln!("error: native source delivery accepts only bounded requests on stdin");
        return std::process::ExitCode::FAILURE;
    }
    match run(io::stdin().lock(), io::stdout().lock()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests;
