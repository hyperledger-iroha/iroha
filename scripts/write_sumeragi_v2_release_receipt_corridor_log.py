# Executed lexically in write_sumeragi_v2_release_receipt.py; do not import directly.

def _test_count_from_log(lines: list[str], kind: str, name: str) -> int:
    if kind == "cargo-focus":
        running = [line for line in lines if line == "running 1 test"]
        results = [
            line
            for line in lines
            if re.fullmatch(
                r"test result: ok\. 1 passed; 0 failed; 0 ignored; "
                r"0 measured; [0-9]+ filtered out; finished in .+",
                line,
            )
            is not None
        ]
        if not running or len(running) != len(results):
            raise ReceiptError(
                f"{name} has an ambiguous Cargo transcript for focused tests"
            )
        return len(results)
    if kind.startswith("cargo-"):
        running = [
            match
            for line in lines
            if (match := re.fullmatch(r"running ([0-9]+) tests?", line))
        ]
        results = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"test result: ok\. ([0-9]+) passed; 0 failed; 0 ignored; "
                    r"0 measured; [0-9]+ filtered out; finished in .+",
                    line,
                )
            )
        ]
        if (
            len(running) != 1
            or len(results) != 1
            or running[0].group(1) != results[0].group(1)
        ):
            raise ReceiptError(f"{name} has an ambiguous Cargo transcript")
        return int(results[0].group(1))
    if kind == "pytest":
        matches = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"([0-9]+) passed in [0-9]+(?:\.[0-9]+)?s", line
                )
            )
        ]
        if len(matches) != 1:
            raise ReceiptError(f"{name} has an ambiguous pytest transcript")
        return int(matches[0].group(1))
    if kind == "node":
        matches = [
            match
            for line in lines
            if (match := re.fullmatch(r"# pass ([0-9]+)", line))
        ]
        if (
            len(matches) != 1
            or lines.count(f"# tests {matches[0].group(1)}") != 1
            or lines.count("# fail 0") != 1
            or lines.count("# cancelled 0") != 1
            or lines.count("# skipped 0") != 1
            or lines.count("# todo 0") != 1
        ):
            raise ReceiptError(f"{name} has an ambiguous Node transcript")
        return int(matches[0].group(1))
    if kind == "native-amx-sdk":
        matches = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"native-amx-v2-grouped-parity surface=[a-z]+ "
                    r"tests=([0-9]+) fixture_sha256=[0-9a-f]{64} "
                    r"suite_source_manifest_sha256=[0-9a-f]{64}",
                    line,
                )
            )
        ]
        if len(matches) != 1:
            raise ReceiptError(
                f"{name} has an ambiguous grouped Native AMX V2 SDK transcript"
            )
        return int(matches[0].group(1))
    if kind == "sdk-diagnostics":
        matches = [
            match
            for line in lines
            if (
                match := re.fullmatch(
                    r"sumeragi-v2-sdk-diagnostics surface=[a-z]+ "
                    r"tests=([0-9]+) "
                    r"suite_source_manifest_sha256=[0-9a-f]{64}",
                    line,
                )
            )
        ]
        if len(matches) != 1:
            raise ReceiptError(
                f"{name} has an ambiguous Sumeragi v2 SDK diagnostics transcript"
            )
        return int(matches[0].group(1))
    if kind == "command":
        return 0
    raise ReceiptError(f"{name} has unknown leg kind {kind}")
