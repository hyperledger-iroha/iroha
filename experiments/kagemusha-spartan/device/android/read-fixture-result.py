#!/usr/bin/env python3
"""Read one result from a completed Android diagnostic instrumentation launch."""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys
from zipfile import BadZipFile, ZipFile

# InstallKagemushaSpartanProgramsV1::MAX_MANIFEST_BYTES.
MAX_MANIFEST_BYTES = 16 * 1024


def selected_paths(apk):
    """Return the expected application-relative paths for one packaged manifest."""
    asset = "assets/spartan-artifacts/parameter-manifest.norito"
    with ZipFile(apk) as archive:
        matches = [entry for entry in archive.infolist() if entry.filename == asset]
        if len(matches) != 1 or not 0 < matches[0].file_size <= MAX_MANIFEST_BYTES:
            raise ValueError("expected one bounded Spartan signed-manifest original")
        digest = hashlib.sha256(archive.read(matches[0])).hexdigest()
    selection = f"spartan-fixtures/{digest}/protected-debit-v1"
    return {
        "spartanManifestSha256": digest,
        "spartanReportPath": f"files/{selection}/spartan-native-runtime-report.txt",
        "spartanCustodyPath": f"no_backup/{selection}",
    }


def launch_field(instrumentation, name):
    prefix = f"INSTRUMENTATION_STATUS: {name}="
    values = [line[len(prefix):] for line in instrumentation.splitlines() if line.startswith(prefix)]
    if len(values) != 1 or not values[0]:
        raise ValueError(f"missing or ambiguous instrumentation field {name}")
    return values[0]


def collect(apk, instrumentation, package, serial, resource, read=subprocess.check_output):
    if not re.fullmatch(r"org\.hyperledger\.iroha\.kagemusha\.enrollmentprobe\.[a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)*", package):
        raise ValueError("expected an isolated enrollment-probe package name")
    expected = selected_paths(apk)
    for name, value in expected.items():
        if launch_field(instrumentation, name) != value:
            raise ValueError(f"recorded {name} differs from the selected APK")
    # First tie the retained report to the supplied launch, including failed native runs.
    base = ["adb", "-s", serial, "exec-out", "run-as", package, "cat"]
    report = read([*base, expected["spartanReportPath"]])
    if hashlib.sha256(report).hexdigest() != launch_field(instrumentation, "spartanReportSha256"):
        raise ValueError("retained report differs from the recorded launch")
    if resource == "report":
        return report
    names = {
        "measurements": ("device-measurements.json", "spartanMeasurementsSha256"),
        "events": ("device-measurements.jsonl", "spartanEventsSha256"),
    }
    filename, checksum_field = names[resource]
    checksum = launch_field(instrumentation, checksum_field)
    data = read([*base, expected["spartanCustodyPath"] + "/" + filename])
    if hashlib.sha256(data).hexdigest() != checksum:
        raise ValueError("retained measurements differ from the recorded launch")
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apk", required=True, type=Path)
    parser.add_argument("--instrumentation", required=True, type=Path,
                        help="saved am instrument -w -r output for this launch")
    parser.add_argument("--package", required=True)
    parser.add_argument("--serial", required=True)
    parser.add_argument("--resource", choices=("report", "measurements", "events"), default="report")
    args = parser.parse_args()
    try:
        data = collect(args.apk, args.instrumentation.read_text(), args.package, args.serial, args.resource)
    except (BadZipFile, KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"{error}\n")
    sys.stdout.buffer.write(data)


if __name__ == "__main__":
    main()
