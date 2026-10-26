//! Bounded retained input custody; digests select bytes without granting authority.
use super::*;

pub(super) struct Inputs(pub(super) Vec<(RetainedFile, FileSnapshot)>);
impl Inputs {
    pub(super) fn read(&mut self, selection: &Original, maximum: usize) -> Result<Vec<u8>> {
        pin(&selection.sha256)?;
        ensure!(
            selection.path.is_absolute(),
            "absolute selected original required"
        );
        let mut held = RetainedFile::open_regular(&selection.path)?;
        let snapshot = held.snapshot()?;
        let length = held.file().metadata()?.len();
        ensure!(
            length > 0 && length <= maximum as u64,
            "bounded original required"
        );
        let mut bytes = Vec::with_capacity(length as usize);
        held.file_mut()
            .take(maximum as u64 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 == length && hash(&bytes) == selection.sha256,
            "selected original differs"
        );
        held.revalidate()?;
        ensure!(held.snapshot()? == snapshot, "original changed");
        self.0.push((held, snapshot));
        Ok(bytes)
    }
    pub(super) fn retain_private(&mut self, path: &Path, maximum: u64) -> Result<()> {
        let held = RetainedFile::open_private(path)?;
        let length = held.file().metadata()?.len();
        ensure!(
            length > 0 && length <= maximum,
            "bounded private original required"
        );
        let snapshot = held.snapshot()?;
        self.0.push((held, snapshot));
        Ok(())
    }
    pub(super) fn program(&mut self, path: &Path, expected: &str, maximum: u64) -> Result<()> {
        pin(expected)?;
        ensure!(path.is_absolute(), "absolute program original required");
        let mut held = RetainedFile::open_regular(path)?;
        let snapshot = held.snapshot()?;
        let length = held.file().metadata()?.len();
        ensure!(
            length > 0 && length <= maximum,
            "bounded program original required"
        );
        let mut hash = Sha256::new();
        let mut buffer = [0; 65536];
        let mut count = 0u64;
        loop {
            let read = held.file_mut().read(&mut buffer)?;
            if read == 0 {
                break;
            }
            count += read as u64;
            ensure!(count <= length, "program grew");
            hash.update(&buffer[..read]);
        }
        ensure!(
            count == length && hex::encode(hash.finalize()) == expected,
            "program selection differs"
        );
        held.revalidate()?;
        ensure!(held.snapshot()? == snapshot, "program changed");
        self.0.push((held, snapshot));
        Ok(())
    }
    pub(super) fn revalidate(&self) -> Result<()> {
        for (held, snapshot) in &self.0 {
            held.revalidate()?;
            ensure!(held.snapshot()? == *snapshot, "selected input changed");
        }
        Ok(())
    }
}
