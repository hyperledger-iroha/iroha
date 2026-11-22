#!/usr/bin/env python3
"""Verify three copied iPhone fixture launches against the packaged programs."""
import argparse
import hashlib
import json
import plistlib
import re
from pathlib import Path

BUNDLE = "jp.co.soramitsu.kagemusha.spartanfixture"
# kagemusha_apple_fixture_namespace_preimage_v1 in apple_debit_key.m, used by FixtureCustody.swift.
NAMESPACE_DOMAIN = b"kagemusha-spartan-fixture-build-namespace-v1"
ANCHOR_SERVICE = "org.hyperledger.iroha.kagemusha.wallet.v1.marker-anchor"


def executable_identity(path):
    """SHA-256 of the signed executable file, and whether that build scopes its namespace by it."""
    marker, overlap, scoped = NAMESPACE_DOMAIN + b"\0", b"", False
    hasher = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            hasher.update(chunk)
            scoped = scoped or marker in overlap + chunk
            overlap = (overlap + chunk)[-(len(marker) - 1):]
    return hasher.hexdigest(), scoped


def fixture_namespace(app):
    """Exact namespace, report fields and data-container paths of a signed fixture app."""
    with (app / "Info.plist").open("rb") as stream:
        info = plistlib.load(stream)
    if info.get("CFBundleIdentifier") != BUNDLE:
        raise ValueError("expected the isolated iPhone fixture app")
    name = info.get("CFBundleExecutable")
    if not isinstance(name, str) or not name or "/" in name or name.startswith("."):
        raise ValueError("invalid fixture executable name")
    executable, scoped = executable_identity(app / name)
    assets = app / "native-device"
    manifest = digest(assets / "parameter-manifest.norito")
    if scoped:
        scope = hashlib.sha256(NAMESPACE_DOMAIN + b"\0" + bytes.fromhex(executable)
                               + bytes.fromhex(manifest)).hexdigest()
        report = {"schema": "kagemusha-spartan-device-runtime-v2", "manifest_sha256": manifest,
                  "build_executable_sha256": executable, "fixture_namespace": scope}
    else:
        # Retained handoffs: the signed manifest digest alone selected the namespace.
        scope = manifest
        report = {"schema": "kagemusha-spartan-device-runtime-v1", "manifest_sha256": manifest}
    return {"scheme": "build-and-manifest-v1" if scoped else "retained-manifest-only",
            "build_executable_sha256": executable, "manifest_sha256": manifest,
            "program_bundle_sha256": digest(assets / "program-bundle.norito"),
            "fixture_namespace": scope, "report": report,
            "report_path": f"Documents/spartan-runtime-result-{scope}.json",
            "history_path": "Library/Application Support/kagemusha-wallet-v1/"
                            f"spartan-runtime-fixture-{scope}/device-measurements.jsonl",
            "payment_key_tag_prefix": f"kgm-ns-w1-{scope}-",
            "approval_key_tag_prefix": f"kgm-ns-debit1-{scope}-",
            "anchor_service": f"{ANCHOR_SERVICE}.{scope}"}


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


def check_retained_funding(history, bundle, retained):
    """Account for funding retained after an interrupted first stage."""
    funded = ["Bootstrap", "Load", "Send"]
    if retained != funded[:len(retained)] or len(retained) > len(funded):
        raise ValueError("invalid retained first-stage operation order")
    if not retained:
        return

    # The last line is the successful launch being checked. Look only at the
    # preceding attempts recorded in that same app-private append-only history.
    completed = []
    with history.open("rb") as stream:
        previous = stream.readline()
        for following in stream:
            try:
                attempt = json.loads(previous)
            except (UnicodeDecodeError, ValueError):
                attempt = None
            previous = following
            if (not isinstance(attempt, dict)
                    or attempt.get("schema") != "kagemusha.experimental.device-observations.v2"
                    or attempt.get("program_bundle_sha256") != bundle
                    or attempt.get("succeeded") is not False):
                completed = []
                continue

            replayed = attempt.get("retained_operations")
            if not isinstance(replayed, list) or replayed != completed[:len(replayed)]:
                completed = []
                continue
            confirmed = False
            for event in attempt["events"]:
                if event["phase"] == "confirmation_wait":
                    confirmed = event["operation"] == "Send" and event["returned_ok"] is True
                if event["phase"] != "execute" or event["returned_ok"] is not True:
                    continue
                operation = event["operation"]
                if (replayed != completed or len(completed) == len(funded)
                        or funded[len(completed)] != operation
                        or (operation == "Send" and not confirmed)):
                    raise ValueError("inconsistent first-stage execution or Send confirmation")
                completed.append(operation)
                replayed = completed.copy()
    if completed != retained:
        raise ValueError("missing evidence for retained first-stage operations")


def verify(app, launches):
    namespace = fixture_namespace(app)
    manifest, bundle = namespace["manifest_sha256"], namespace["program_bundle_sha256"]
    if len(launches) != 3:
        raise ValueError("expected the Send, Receive and restart launches in order")
    prior_history = None
    originals = None
    results = []
    funded = ["Bootstrap", "Load", "Send"]
    for index, folder in enumerate(launches):
        report = json.loads((folder / "result.json").read_bytes())
        if any(report.get(key) != value for key, value in namespace["report"].items()):
            raise ValueError("report selects a different fixture schema, build namespace or signed manifest")
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

        retained = metrics["retained_operations"]
        if not isinstance(retained, list):
            raise ValueError("invalid retained operations")
        if index == 0:
            check_retained_funding(folder / "device-measurements.jsonl", bundle, retained)
            expected_events = []
            for operation in funded[len(retained):]:
                if operation == "Send":
                    expected_events.append(("Send", "confirmation_wait"))
                expected_events.append((operation, "execute"))
        else:
            if retained != (funded if index == 1 else [*funded, "Receive"]):
                raise ValueError(f"unexpected retained operations in launch {index + 1}")
            expected_events = [("Receive", "execute")] if index == 1 else []

        observed_events = [(e["operation"], e["phase"]) for e in events
                           if e["phase"] in ("execute", "confirmation_wait")]
        if (observed_events != expected_events
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
            "payments": payments, "receive": receive,
            **{key: namespace[key] for key in ("scheme", "build_executable_sha256", "fixture_namespace")}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", required=True, type=Path)
    parser.add_argument("--describe", action="store_true",
                        help="print the app's namespace and data-container paths instead")
    parser.add_argument("launch", nargs="*", type=Path, metavar="DIRECTORY")
    args = parser.parse_args()
    if args.describe != (not args.launch) or (args.launch and len(args.launch) != 3):
        parser.error("give --describe, or the three launch directories")
    try:
        print(json.dumps(fixture_namespace(args.app) if args.describe else verify(args.app, args.launch), indent=2))
    except (KeyError, OSError, TypeError, ValueError) as error:
        parser.exit(1, f"{error}\n")


if __name__ == "__main__":
    main()
