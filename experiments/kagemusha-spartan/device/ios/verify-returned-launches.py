#!/usr/bin/env python3
"""Verify three copied iPhone fixture launches against the packaged programs."""
import argparse
import hashlib
import json
import plistlib
import re
from pathlib import Path


def digest(path):
    with path.open("rb") as stream:
        hasher = hashlib.sha256()
        for chunk in iter(lambda: stream.read(1 << 16), b""):
            hasher.update(chunk)
        return hasher.hexdigest()


def history_digests(path, measurement):
    """Hash the history before and after the exact line returned by Native."""
    line = measurement.encode("utf-8") + b"\n"
    remaining = path.stat().st_size - len(line)
    if remaining < 0:
        raise ValueError("copied history is shorter than the returned measurement")
    prefix = hashlib.sha256()
    previous = b"\n"
    with path.open("rb") as stream:
        while remaining:
            chunk = stream.read(min(1 << 16, remaining))
            if not chunk:
                raise ValueError("incomplete history prefix")
            prefix.update(chunk)
            previous = chunk[-1:]
            remaining -= len(chunk)
        if previous != b"\n" or stream.read(len(line) + 1) != line:
            raise ValueError("last history record differs from the copied launch report")
    before = prefix.hexdigest()
    prefix.update(line)
    return before, prefix.hexdigest()


def verify(app, launches):
    with (app / "Info.plist").open("rb") as stream:
        if plistlib.load(stream).get("CFBundleIdentifier") != "jp.co.soramitsu.kagemusha.spartanfixture":
            raise ValueError("expected the isolated iPhone fixture app")
    assets = app / "native-device"
    manifest = digest(assets / "parameter-manifest.norito")
    bundle = digest(assets / "program-bundle.norito")
    if len(launches) != 3:
        raise ValueError("expected the Send, Receive and restart launches in order")
    prior_history = None
    originals = None
    results = []
    funded = ["Bootstrap", "Load", "Send"]
    expected_retained = ([], funded, [*funded, "Receive"])
    expected_executions = (funded, ["Receive"], [])
    expected_confirmations = (["Send"], [], [])
    for index, folder in enumerate(launches):
        report = json.loads((folder / "result.json").read_bytes())
        if (report["schema"] != "kagemusha-spartan-device-runtime-v1"
                or report["manifest_sha256"] != manifest):
            raise ValueError("report selects a different fixture schema or signed manifest")
        summary, separator, line = report["result"].partition("\n")
        expected = "PASS: " + ("stage 1 of 2: " if index == 0 else "") + "device hardware keys."
        if not separator or not summary.startswith(expected):
            raise ValueError(f"launch {index + 1} has no successful iPhone Native summary")
        metrics = json.loads(line)
        if (metrics["schema"] != "kagemusha.experimental.device-observations.v2"
                or metrics["succeeded"] is not True
                or metrics["program_bundle_sha256"] != bundle):
            raise ValueError("Native measurement differs from the packaged programs")
        before, history = history_digests(folder / "device-measurements.jsonl", line)
        if prior_history is not None and before != prior_history:
            raise ValueError("expected consecutive histories from the same custody")
        prior_history = history
        observed = tuple((metrics[side + "_payment_bytes"], metrics[side + "_payment_sha256"])
                         for side in ("outgoing", "incoming"))
        for length, sha in observed:
            if (type(length) is not int
                    or length <= 0 or not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{64}", sha)):
                raise ValueError("invalid retained Payment identity")
        if observed[0][1] == observed[1][1] or (originals is not None and observed != originals):
            raise ValueError("retained Payments differ across launches")
        originals = observed
        events = metrics["events"]
        if any(event["returned_ok"] is not True for event in events):
            raise ValueError(f"unsuccessful Native event in launch {index + 1}")

        def operations(phase):
            return [e["operation"] for e in events if e["phase"] == phase]

        if (metrics["retained_operations"] != expected_retained[index]
                or operations("execute") != expected_executions[index]
                or operations("confirmation_wait") != expected_confirmations[index]
                or operations("credit_check_drill") != ([] if index == 0 else ["Receive"])):
            raise ValueError(f"unexpected execution, debit confirmation or credit check in launch {index + 1}")
        if index == 1:
            flow = metrics["receive_normal_flow_us"]
            sampled = metrics["process_lifetime_peak_rss_bytes"]
            if type(flow) is not int or type(sampled) is not int or min(flow, sampled) <= 0:
                raise ValueError("missing complete Receive resources")
            observed, drills, overhead = (metrics[name] for name in ("receive_observed_flow_us",
                "receive_test_drill_us", "receive_instrumentation_us"))
            if (any(type(v) is not int or v < 0 for v in (observed, drills, overhead))
                    or flow != observed - drills - overhead):
                raise ValueError("Receive timing differs from observed phases")
            receive = {"normal_flow_us": flow, "process_lifetime_peak_rss_bytes": sampled}
        results.append({"history_sha256": history, "measurement_sha256": hashlib.sha256(line.encode()).hexdigest()})
    payments = {side: {"bytes": length, "sha256": sha}
                for side, (length, sha) in zip(("outgoing", "incoming"), originals)}
    return {"manifest_sha256": manifest, "program_bundle_sha256": bundle, "launches": results,
            "payments": payments, "receive": receive}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True, type=Path)
    parser.add_argument("launch", nargs=3, type=Path, metavar="DIRECTORY")
    args = parser.parse_args()
    try:
        print(json.dumps(verify(args.app, args.launch), indent=2))
    except (KeyError, OSError, TypeError, ValueError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
