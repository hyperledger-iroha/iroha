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
