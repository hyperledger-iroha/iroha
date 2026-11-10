#!/usr/bin/env python3
"""Opt-in qualification of an explicitly pinned native C checkpoint bridge.

Prerequisites: Python 3.11+ (standard library; validated with Python 3.12) on
macOS/Linux. Native mode needs a reviewed host/architecture-compatible bridge
library and its independently admitted source/build receipt. --admit-only loads
no native library and performs public input checks only.

Required environment for either mode:
  BOI_ACTIVATION_FIXTURE_DIR: canonical external directory of generated originals.
  BOI_ACTIVATION_MANIFEST_SHA256: independently pinned generated manifest digest.
  BOI_ACTIVATION_PRODUCER_RECEIPT: canonical external producer receipt file.
  BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256: independently pinned receipt digest.
CLI arguments supply the reviewed source root/commit/tree, fresh external output,
and, in native mode, exact library and artifact-receipt paths and SHA256 pins.

Public JSON/file admission is not native proof verification. The supplied library
owns all Norito decoding and finality checks. No fixture generation or signing.
Existing outputs are never overwritten. Use --help for options and README.md for
ordinary-suite discovery, producer admission and dedicated qualification rules.
"""
import argparse
import ctypes as C
import hashlib
import json
import os
from pathlib import Path
import re
import stat

MIB = 1024 * 1024
MAX_ORIGINAL = 68 * MIB
TEST_NAME = "runtime::taira_activation::tests::export_time_ironwood_activation_originals_for_native_consumers"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def hex_pin(value, size=64):
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{%d}" % size, value)
            and value != "0" * size, "invalid lowercase nonzero identity")
    return value


def no_duplicates(pairs):
    obj = {}
    for key, value in pairs:
        require(key not in obj, "duplicate JSON key")
        obj[key] = value
    return obj


def json_original(data):
    return json.loads(data.decode("utf-8"), object_pairs_hook=no_duplicates,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError("nonfinite JSON")))


def record(value, fields):
    require(type(value) is dict and set(value) == set(fields), "closed JSON object fields")
    return value


def external_path(raw, repo):
    path = Path(raw)
    require(path.is_absolute(), "input path must be absolute")
    resolved = path.resolve(strict=True)
    require(path == resolved, "input path must be canonical and have no symlink components")
    require(not resolved.is_relative_to(repo.resolve(strict=True)), "input must be external to source checkout")
    return resolved


def read_regular(path, limit):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= limit,
                "expected bounded regular original: " + str(path))
        data = bytearray()
        while len(data) < before.st_size:
            chunk = os.read(fd, min(1024 * 1024, before.st_size - len(data)))
            require(bool(chunk), "original changed during read")
            data.extend(chunk)
        require(os.read(fd, 1) == b"", "original grew during read")
        after = os.fstat(fd)
        require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
                == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns),
                "original changed during read")
        return bytes(data)
    finally:
        os.close(fd)


def hash_regular(path, limit):
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    try:
        before = os.fstat(fd)
        require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= limit, "bounded artifact file")
        hasher, size = hashlib.sha256(), 0
        while chunk := os.read(fd, MIB):
            size += len(chunk)
            require(size <= limit, "artifact exceeded bound")
            hasher.update(chunk)
        after = os.fstat(fd)
        require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
                == (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
                and size == before.st_size, "artifact changed during hash")
        return hasher.hexdigest()
    finally:
        os.close(fd)


def load_fixture(repo, directory, manifest_pin, receipt_path, receipt_pin):
    root = external_path(directory, repo)
    require(root.is_dir(), "fixture root must be a directory")
    require(root.stat().st_uid == os.geteuid() and not root.stat().st_mode & 0o022,
            "fixture root must be owner-controlled and not writable by group/others")
    receipt_file = external_path(receipt_path, repo)
    require(not receipt_file.is_relative_to(root), "producer receipt must be outside original directory")
    manifest_bytes = read_regular(root / "manifest.json", 65536)
    receipt_bytes = read_regular(receipt_file, 65536)
    require(digest(manifest_bytes) == hex_pin(manifest_pin), "manifest pin mismatch")
    require(digest(receipt_bytes) == hex_pin(receipt_pin), "producer receipt pin mismatch")
    manifest, receipt = map(json_original, (manifest_bytes, receipt_bytes))
    record(manifest, ("schema", "fixtureKind", "patchBaseCommit", "patchBaseCommitIsProducerSource", "consumerAdmission",
        "networkId", "chainId", "anchorHeight", "finalHeight", "scheduleHeight", "activationHeight", "bootstrapHeight",
        "anchorContextId", "finalContextId", "predecessorSelection", "successorSelection",
        "checkpoint", "promotedCheckpoint", "world", "proofs"))
    record(receipt, ("schema", "source", "testExecutable", "buildReceiptSha256", "run", "fixtureManifestSha256", "files"))
    require(manifest["schema"] == "iroha.time-ironwood-native-consumer-fixture.v1", "fixture schema")
    require(manifest["fixtureKind"] == "certified-synthetic-activation-history", "fixture kind")
    require(manifest["patchBaseCommitIsProducerSource"] is False, "patch base is not producer")
    require(manifest["consumerAdmission"] == "requires-external-producer-receipt", "producer admission required")
    hex_pin(manifest["patchBaseCommit"], 40)
    hex_pin(manifest["predecessorSelection"])
    hex_pin(manifest["successorSelection"])
    require(receipt["schema"] == "boi.time-ironwood-fixture-producer.v1", "producer receipt schema")
    require(receipt["fixtureManifestSha256"] == manifest_pin, "receipt/manifest join")
    source = record(receipt["source"], ("commit", "tree", "signedHeadVerified", "cleanBefore", "cleanAfter"))
    hex_pin(source["commit"], 40)
    hex_pin(source["tree"], 40)
    require(all(source[key] is True for key in ("signedHeadVerified", "cleanBefore", "cleanAfter")),
            "reviewed signed clean source evidence required")
    executable = record(receipt["testExecutable"], ("sha256", "size"))
    hex_pin(executable["sha256"])
    require(type(executable["size"]) is int and executable["size"] > 0, "test executable identity")
    hex_pin(receipt["buildReceiptSha256"])
    run = record(receipt["run"], ("testName", "exitCode", "passed", "failed", "ignored"))
    require(run["testName"] == TEST_NAME, "exporter test identity")
    require(all(type(run[key]) is int and run[key] == expected
                for key, expected in (("exitCode", 0), ("passed", 1), ("failed", 0), ("ignored", 0))),
            "successful exact exporter run required")
    heights = {}
    for key in ("anchorHeight", "scheduleHeight", "activationHeight", "bootstrapHeight", "finalHeight"):
        value = manifest[key]
        require(isinstance(value, str) and re.fullmatch(r"[1-9][0-9]*", value), "canonical positive height")
        heights[key] = int(value)
        require(heights[key] < 2**64 - 1, "height range")
    require(heights["anchorHeight"] < heights["scheduleHeight"] < heights["activationHeight"]
            < heights["bootstrapHeight"] == heights["finalHeight"] == heights["activationHeight"] + 1,
            "activation history heights")
    hex_pin(manifest["anchorContextId"])
    hex_pin(manifest["finalContextId"])
    for key in ("networkId", "chainId"):
        require(isinstance(manifest[key], str) and 0 < len(manifest[key].encode()) <= 1024,
                "bounded native network/chain label")
    proofs = manifest["proofs"]
    require(isinstance(proofs, list) and 2 <= len(proofs) <= 4096, "proof count bound")
    require(len(proofs) == heights["finalHeight"] - heights["anchorHeight"] + 1, "complete proof history")
    names = ["anchor-checkpoint.norito", "promoted-checkpoint.norito", "world.norito"]
    names += [f"proof-{index:04}.norito" for index in range(len(proofs))]
    references = [manifest[key] for key in ("checkpoint", "promotedCheckpoint", "world")] + proofs
    originals, identities = {}, {}
    for expected_name, ref in zip(names, references, strict=True):
        require(set(ref) == {"name", "size", "sha256"} and ref["name"] == expected_name, "original name/shape")
        require(isinstance(ref["size"], str) and re.fullmatch(r"[1-9][0-9]*", ref["size"]), "canonical size")
        size = int(ref["size"])
        require(size <= (32 * MIB if expected_name == "world.norito" else MAX_ORIGINAL), "original size cap")
        identities[expected_name] = (size, hex_pin(ref["sha256"]))
    # Reject declared aggregate excess before reading or retaining any large proof.
    require(sum(identities[ref["name"]][0] for ref in proofs) <= 16 * MIB, "proof page byte bound")
    require({path.name for path in root.iterdir()} == set(names) | {"manifest.json"}, "unexpected fixture contents")
    recorded = {}
    for item in receipt["files"]:
        require(set(item) == {"name", "size", "sha256"} and item["name"] not in recorded, "receipt file shape/duplicate")
        require(type(item["size"]) is int and item["size"] > 0, "receipt file size")
        recorded[item["name"]] = (item["size"], hex_pin(item["sha256"]))
    require(recorded == identities, "receipt originals differ from manifest")
    for name, (size, sha) in identities.items():
        data = read_regular(root / name, size)
        require(len(data) == size and digest(data) == sha, "original identity mismatch")
        originals[name] = data
    return manifest, receipt, originals


class NativeBridge:
    def __init__(self, library):
        self.library = C.CDLL(str(library))
        self.call = self.library.connect_norito_advance_fi_checkpoint_v1
        p, n = C.c_void_p, C.c_ulong
        self.call.argtypes = [p, n, C.POINTER(p), C.POINTER(n), n,
                              p, n, p, n, p, n, p, n, p, n,
                              C.POINTER(p), C.POINTER(n), C.POINTER(p), C.POINTER(n)]
        self.call.restype = C.c_int
        self.free = self.library.connect_norito_free
        self.free.argtypes, self.free.restype = [p], None

    def advance(self, checkpoint, proofs, network, chain, sha, height, context):
        values = [checkpoint, *proofs, network.encode(), chain.encode(), sha, str(height).encode(), context]
        buffers = [C.create_string_buffer(value) for value in values]
        count = len(proofs)
        pointers = (C.c_void_p * count)(*[C.cast(buf, C.c_void_p).value for buf in buffers[1:1 + count]])
        lengths = (C.c_ulong * count)(*[len(proof) for proof in proofs])
        tail = []
        for value, buffer in zip(values[1 + count:], buffers[1 + count:]):
            tail.extend((C.cast(buffer, C.c_void_p), len(value)))
        out, meta, out_len, meta_len = C.c_void_p(), C.c_void_p(), C.c_ulong(123), C.c_ulong(123)
        status = self.call(C.cast(buffers[0], C.c_void_p), len(checkpoint), pointers, lengths, count,
                           *tail, C.byref(out), C.byref(out_len), C.byref(meta), C.byref(meta_len))
        if status != 0:
            require(status == -512 and not out.value and not meta.value and out_len.value == meta_len.value == 0,
                    "native refusal must clear both owned outputs")
            return None
        try:
            require(out.value and meta.value and 0 < out_len.value <= MAX_ORIGINAL and 0 < meta_len.value <= 4096,
                    "native success output bounds")
            return C.string_at(out, out_len.value), C.string_at(meta, meta_len.value)
        finally:
            self.free(out)
            self.free(meta)


def run_native(bridge, manifest, originals):
    checkpoint = originals[manifest["checkpoint"]["name"]]
    proofs = [originals[ref["name"]] for ref in manifest["proofs"]]
    args = dict(checkpoint=checkpoint, proofs=proofs, network=manifest["networkId"], chain=manifest["chainId"],
                sha=bytes.fromhex(manifest["checkpoint"]["sha256"]), height=int(manifest["anchorHeight"]),
                context=bytes.fromhex(manifest["anchorContextId"]))
    positive = bridge.advance(**args)
    require(positive is not None, "genuine activation history must pass native C bridge")
    output, metadata_bytes = positive
    require(output == originals[manifest["promotedCheckpoint"]["name"]], "promoted original must be byte-identical")
    metadata = json_original(metadata_bytes)
    expected = {"schema": "iroha.fi-checkpoint-advance.verified.v1", "networkId": manifest["networkId"],
                "chainId": manifest["chainId"], "anchorHeight": manifest["anchorHeight"],
                "height": manifest["finalHeight"], "proofCount": str(len(proofs)),
                "checkpointSha256": manifest["promotedCheckpoint"]["sha256"], "contextId": manifest["finalContextId"]}
    require(set(metadata) == set(expected) | {"blockHash"}, "exact native metadata keys")
    require(all(metadata[key] == value for key, value in expected.items()), "native metadata equality")
    hex_pin(metadata["blockHash"])
    flip = lambda value: bytes([value[0] ^ 1]) + value[1:]
    bad_proof = [*proofs]
    bad_proof[1] = flip(bad_proof[1])
    truncated = [*proofs]
    truncated[1] = truncated[1][:-1]
    # Existing native fixtures/governance/runtime_change_set_v1.json canonical
    # checked literal; use a valid foreign identity, not a malformed text label.
    foreign_network = "hash:" + "11" * 32 + "#4667"
    require(foreign_network != manifest["networkId"], "foreign network control must differ")
    cases = {
        "mutated-proof": {"proofs": bad_proof}, "truncated-proof": {"proofs": truncated},
        "missing-anchor": {"proofs": proofs[1:]}, "gap": {"proofs": proofs[:1] + proofs[2:]},
        "no-successor": {"proofs": proofs[:1]}, "foreign-network": {"network": foreign_network},
        "foreign-chain": {"chain": manifest["chainId"] + "-foreign"},
        "foreign-checkpoint-digest": {"sha": flip(args["sha"])},
        "foreign-anchor-height": {"height": args["height"] + 1},
        "foreign-anchor-context": {"context": flip(args["context"])},
        "mutated-checkpoint-matching-digest": {"checkpoint": flip(checkpoint),
                                               "sha": hashlib.sha256(flip(checkpoint)).digest()},
    }
    for name, changes in cases.items():
        require(bridge.advance(**(args | changes)) is None, "native accepted " + name)
    return ["genuine-activation-original-equality", *cases]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--admit-only", action="store_true", help="validate pinned public inputs only; never load native code")
    parser.add_argument("--source-root", required=True, type=Path)
    parser.add_argument("--library", type=Path)
    parser.add_argument("--library-sha256")
    parser.add_argument("--artifact-receipt", type=Path)
    parser.add_argument("--artifact-receipt-sha256")
    parser.add_argument("--source-commit", required=True)
    parser.add_argument("--source-tree", required=True)
    parser.add_argument("--output", required=True, type=Path)
    options = parser.parse_args()
    manifest, receipt, originals = load_fixture(options.source_root,
        os.environ["BOI_ACTIVATION_FIXTURE_DIR"], os.environ["BOI_ACTIVATION_MANIFEST_SHA256"],
        os.environ["BOI_ACTIVATION_PRODUCER_RECEIPT"], os.environ["BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256"])
    require(receipt["source"]["commit"] == hex_pin(options.source_commit, 40)
            and receipt["source"]["tree"] == hex_pin(options.source_tree, 40), "expected reviewed producer source")
    require(options.output.is_absolute() and options.output.parent == options.output.parent.resolve(strict=True),
            "absolute canonical output parent required")
    require(not options.output.is_relative_to(options.source_root.resolve(strict=True))
            and not options.output.is_relative_to(Path(os.environ["BOI_ACTIVATION_FIXTURE_DIR"])),
            "result must be outside source and immutable fixture directories")
    require(not options.output.exists() and not options.output.is_symlink(), "new output receipt required")
    if options.admit_only:
        require(all(value is None for value in (options.library, options.library_sha256,
                                                options.artifact_receipt, options.artifact_receipt_sha256)),
                "admission-only mode accepts no native artifact arguments")
        report = {"schema": "boi.time-ironwood-public-input-admission.v1", "status": "INPUTS_ADMITTED_ONLY",
                  "fixtureManifestSha256": os.environ["BOI_ACTIVATION_MANIFEST_SHA256"],
                  "producerReceiptSha256": os.environ["BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256"],
                  "sourceCommit": options.source_commit, "sourceTree": options.source_tree,
                  "nativeLoaded": False, "nativeChecks": 0,
                  "limits": "Public byte/shape checks only; independent producer review and actual native execution required."}
        with options.output.open("x") as file:
            json.dump(report, file, indent=2)
            file.write("\n")
        print(json.dumps({"status": report["status"], "nativeChecks": 0, "receipt": str(options.output)}))
        return
    require(all((options.library, options.library_sha256, options.artifact_receipt, options.artifact_receipt_sha256)),
            "native mode requires library and artifact receipt paths and pins")
    library = external_path(options.library, options.source_root)
    artifact = external_path(options.artifact_receipt, options.source_root)
    require(hash_regular(library, 2 * 1024 * MIB) == hex_pin(options.library_sha256), "library pin")
    require(hash_regular(artifact, 4 * MIB) == hex_pin(options.artifact_receipt_sha256), "artifact receipt pin")
    # The independently reviewed artifact receipt must establish source/build admission.
    # This driver pins its original bytes; it does not invent a second producer verifier.
    checks = run_native(NativeBridge(library), manifest, originals)
    require(hash_regular(library, 2 * 1024 * MIB) == options.library_sha256, "library changed during run")
    report = {"schema": "boi.time-ironwood-native-c-consumer-result.v1", "status": "PASSED",
              "library": str(library), "librarySha256": options.library_sha256,
              "artifactReceipt": str(artifact), "artifactReceiptSha256": options.artifact_receipt_sha256,
              "fixtureManifestSha256": os.environ["BOI_ACTIVATION_MANIFEST_SHA256"],
              "producerReceiptSha256": os.environ["BOI_ACTIVATION_PRODUCER_RECEIPT_SHA256"],
              "sourceCommit": options.source_commit, "sourceTree": options.source_tree,
              "checks": checks, "worldEntryPointQualified": False,
              "limits": "Supplied host C library only; separate actual Swift/JNI artifact runs, World entrypoint, physical-device and live activation evidence remain required."}
    with options.output.open("x") as file:
        json.dump(report, file, indent=2)
        file.write("\n")
    print(json.dumps({"status": "PASSED", "checks": len(checks), "receipt": str(options.output)}))


if __name__ == "__main__":
    main()
