// Direct, no-follow queue-plan journal file opening helpers.

fn configure_direct_regular_open(options: &mut OpenOptions) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
}

fn open_new_regular(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.create_new(true).read(true).write(true);
    configure_direct_regular_open(&mut options);
    let file = options.open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn open_regular_append(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let mut options = OpenOptions::new();
    options.append(true).read(true);
    configure_direct_regular_open(&mut options);
    let file = options.open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn open_regular_read_write(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    configure_direct_regular_open(&mut options);
    let file = options.open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn open_regular_read(path: &Path) -> io::Result<File> {
    validate_regular_path(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    configure_direct_regular_open(&mut options);
    let file = options.open(path)?;
    verify_open_regular_path(path, &file)?;
    Ok(file)
}

fn poisoned_journal_error() -> io::Error {
    io::Error::other("queue plan journal is poisoned after an ambiguous durability boundary")
}

fn invalid_data(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn invalid_input(error: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error.to_string())
}
