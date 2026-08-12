# Executed lexically in write_sumeragi_v2_release_receipt.py.

def _complete_write(descriptor: int, data: bytes) -> None:
    view = memoryview(data)
    while view:
        try:
            written = os.write(descriptor, view)
        except InterruptedError:
            continue
        if written <= 0:
            raise ReceiptError("terminal receipt write made no progress")
        view = view[written:]


def _owned_unlink_name(
    directory_fd: int, name: str, device: int, inode: int
) -> bool:
    try:
        metadata = os.stat(name, dir_fd=directory_fd, follow_symlinks=False)
    except (FileNotFoundError, OSError):
        return False
    if (
        not stat.S_ISREG(metadata.st_mode)
        or (metadata.st_dev, metadata.st_ino) != (device, inode)
    ):
        return False
    try:
        os.unlink(name, dir_fd=directory_fd)
    except OSError:
        return False
    return True
