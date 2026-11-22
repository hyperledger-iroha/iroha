#!/usr/bin/env python3
"""Export the four released originals from the isolated two-wallet host fixture."""
import argparse
from hashlib import sha256
import json
from pathlib import Path
import re
import sys
from tempfile import TemporaryDirectory

# Use the SDK from the same checkout as the fixture and compiler.
sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "python/norito_py/src"))
from norito import (
    SchemaDescriptor, StructAdapter, StructField, WireLayout, fixed_bytes,
    from_bytes_view, seq, u8, u16, u128,
)

FROZEN_SCHEMA = SchemaDescriptor.type_name("iroha_core::zk::kagemusha_wallet_advance_v1::FrozenFileV1")
FROZEN = StructAdapter([
    StructField("version", u16()),
    StructField("written_boot_id", fixed_bytes(32)),
    StructField("selected_generation", u128()),
    StructField("frame", seq(u8())),
], framed=True)
MAX_FRAME = 4 * 1024 * 1024 + 16 * 1024
MAX_ENVELOPE = MAX_FRAME + 512


def read_bounded(path, limit):
    with path.open("rb") as stream:
        data = stream.read(limit + 1)
    if len(data) > limit:
        raise ValueError(f"{path} exceeds its fixture bound")
    return data


def frozen_files(paths, body_type):
    found = {}
    for path in paths:
        data = read_bounded(path, MAX_ENVELOPE)
        # Check checksum, compression and schema before allocating the nested vector.
        view = from_bytes_view(data, schema=FROZEN_SCHEMA, layout=WireLayout.CURRENT)
        envelope = view.decode(FROZEN)
        generation = envelope["selected_generation"]
        if envelope["version"] != 1 or generation == 0 or generation in found:
            raise ValueError(f"{path} has an unexpected version or selected generation")
        frame = bytes(envelope["frame"])
        if len(frame) > MAX_FRAME:
            raise ValueError(f"{path} contains an oversized frame")
        schema = SchemaDescriptor.type_name(f"iroha.research.kagemusha_spartan.native.{body_type}")
        from_bytes_view(frame, schema=schema, layout=WireLayout.CURRENT)
        found[generation] = (path, frame)
    return found


def demo_originals(wallet):
    if not read_bounded(wallet / "summary-launch-3.txt", 1024 * 1024).startswith(b"host software keys."):
        raise ValueError("expected the third launch of the software host fixture")
    keys = read_bounded(wallet / "host-software-keys.bin", 65)
    report = json.loads(read_bounded(wallet / "device-measurements-launch-3.json", 1024 * 1024))
    if (len(keys) != 65 or report.get("succeeded") is not True
            or report.get("retained_operations") != ["Bootstrap", "Load", "Send", "Receive"]):
        raise ValueError("expected the complete four-operation software wallet")
    for file, key in [("complete-payment.bin", "outgoing_payment_sha256"),
                      ("incoming-payment.bin", "incoming_payment_sha256")]:
        if sha256(read_bounded(wallet / file, MAX_FRAME)).hexdigest() != report.get(key):
            raise ValueError(f"{file} differs from the recovered host Payment")
    slots = list((wallet / "custody/slots").iterdir())
    if len(slots) != 1 or slots[0].name != keys[:32].hex():
        raise ValueError("the software fixture must contain only its recorded simulated slot")
    slot = slots[0]
    capsules = frozen_files((slot / "capsules").glob("*.cap"), "Capsule.v2")
    completions = frozen_files((slot / "completion").glob("*.cr"), "Completion.v1")
    if len(capsules) != 4 or capsules.keys() != completions.keys():
        raise ValueError("expected four matching capsule/completion selected generations")
    files = {}
    for generation, (record, frame) in completions.items():
        if not re.fullmatch(r"[0-9a-f]{64}", record.stem):
            raise ValueError(f"invalid operation filename {record.name}")
        files[record.name] = frame
        files[record.with_suffix(".cap").name] = capsules[generation][1]

    # The storage owner prefixes every unencrypted public peer original with the
    # archive-object digest. The native compiler will authenticate the Request.
    for record in (slot / "archive").glob("w-*.rec"):
        with record.open("rb") as stream:
            prefix = stream.read(36)
            if prefix[32:] != b"IPM1":
                continue
            tail = stream.read(MAX_FRAME + 1)
        if len(tail) > MAX_FRAME - 4:
            raise ValueError(f"{record} contains an oversized peer original")
        data = b"IPM1" + tail
        hashed = b"iroha:kagemusha:wallet-provider:v1:archive-object\x00" + len(data).to_bytes(8, "little") + data
        if sha256(hashed).digest() != prefix[:32]:
            raise ValueError(f"{record} has an invalid digest")
        files[record.with_suffix(".ipm1").name] = data
    if len({value for name, value in files.items() if name.endswith(".ipm1")}) < 2:
        raise ValueError("expected the original Request and a distinct comparison original")
    return files


def export(wallet, output):
    if output.exists():
        raise ValueError(f"output already exists: {output}")
    files = demo_originals(wallet)
    with TemporaryDirectory(prefix=".archive-originals-", dir=output.parent) as work:
        ready = Path(work) / "originals"
        ready.mkdir(mode=0o700)
        for name, data in files.items():
            (ready / name).write_bytes(data)
        if output.exists():
            raise ValueError(f"output already exists: {output}")
        ready.rename(output)
    print(f"Exported four capsule/completion pairs and {len(files) - 8} peer originals to {output}")


if __name__ == "__main__":
    args = argparse.ArgumentParser(description=__doc__)
    args.add_argument("wallet", type=Path, help="completed two_wallet_device_flow_on_host output")
    args.add_argument("output", type=Path, help="new directory for KAGEMUSHA_SPARTAN_ARCHIVE_RAW_ORIGINALS")
    selected = args.parse_args()
    export(selected.wallet, selected.output)
