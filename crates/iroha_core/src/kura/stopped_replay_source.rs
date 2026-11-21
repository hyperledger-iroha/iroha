// Included at Kura module scope. Retains the stopped source, never a live-copy lease.

pub(crate) struct StoppedReplayJournals {
    files: [(PathBuf, iroha_fs::RetainedFile, iroha_fs::FileSnapshot); 4],
}

impl StoppedReplayJournals {
    pub(crate) fn revalidate(&self) -> Result<()> {
        for (path, file, snapshot) in &self.files {
            file.revalidate()
                .map_err(|error| Error::IO(error, path.clone()))?;
            if file
                .snapshot()
                .map_err(|error| Error::IO(error, path.clone()))?
                != *snapshot
            {
                return Err(Error::IO(
                    std::io::Error::new(
                        ErrorKind::InvalidData,
                        "original stopped replay journal changed",
                    ),
                    path.clone(),
                ));
            }
        }
        Ok(())
    }
}

impl Kura {
    pub(crate) fn retain_stopped_replay_journals(&self) -> Result<StoppedReplayJournals> {
        if !self.is_locked_read_only_source() {
            return Err(Error::IO(
                std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "stopped replay journals require the original read-only root lock",
                ),
                self.store_root(),
            ));
        }
        let directory = self.active_blocks_dir.lock().clone();
        let bind = |name: &str| -> Result<_> {
            let path = directory.join(name);
            let file = iroha_fs::RetainedFile::open_regular(&path)
                .map_err(|error| Error::IO(error, path.clone()))?;
            let snapshot = file
                .snapshot()
                .map_err(|error| Error::IO(error, path.clone()))?;
            Ok((path, file, snapshot))
        };
        let source = StoppedReplayJournals {
            files: [
                bind(DATA_FILE_NAME)?,
                bind(INDEX_FILE_NAME)?,
                bind(HASHES_FILE_NAME)?,
                bind(COUNT_FILE_NAME)?,
            ],
        };
        source.revalidate()?;
        Ok(source)
    }
}
