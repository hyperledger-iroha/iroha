#!/usr/bin/env python3
"""Ordered requested-allocation grammar for the standalone ballot verifier.

Requires Python 3.11+, the reviewed repository sources and Rust 1.99.1 allocation
semantics. No environment variables, external packages, builds or mutations are
used. The JSON CLI is read-only. These traces cover Rust heap requests, including
allocator-default allocate/copy/free reallocation; they are not a native-memory,
stack, whole-process or production qualification certificate. Consumers must
check source_inventory(), enforce trace_assumptions(), and prove each compressed
repeat restores the allocator's complete state, not just its live-byte count.
"""

from __future__ import annotations

import argparse
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUST_REVISION = "58cf8f9c35b355199727627d07ae34a4e2846d67"
ROWS = {"candidate": (33, 8, 704), "comparator": (22, 6, 696)}
WITNESS_BITS = {"registration": 35840, "vote": 53504}
ZKP = "crates/iroha_zkp_vole/src/"
BALLOT = "crates/iroha_parliament_ballot_proof/src/"
SOURCE_DIRECTORIES = (ZKP, BALLOT, "vendor/aes/src/")
# A changed pin requires reviewing the trace against the changed implementation.
REVIEWED_SOURCE_SHA256 = "d163a919743012dd9bc26fad6d4d104b96e0f9454db2647da18c41124f230d77"
RUST_SOURCE_SHA256: dict[str, str] = {
    "alloc/alloc.rs": "3949baaebc3310d1a88a097d3b31b02445bfcad74b196ea4f6ea776de368ab97",
    "alloc/raw_vec/mod.rs": "cda0e96514e65a55b8ed559b02b7bc5f2dcb7854695a501ac0b03b0eeab0361c",
    "alloc/slice.rs": "67228d05babee451c4f29b12fb6e922d1b7be19cf8bdc7efb21edd02c8a69a3e",
    "alloc/vec/mod.rs": "e0c798ff16cf1770ef3266d28440d93dd65402b3afe1df4e184f15af17fb1e3e",
    "alloc/vec/spec_extend.rs": "a0816ad0f39b947889771bf06930bf8e0c4ce794846d83863349697ec7c415ec",
    "alloc/vec/spec_from_elem.rs": "062e397b30aa43082d2f8abf3b8ec9869de2efd6ca573e1113a2ac5f4729489c",
    "alloc/vec/spec_from_iter.rs": "393d1f28cfc8d28d80e142cef6b612aae9b484160969cfa0c9f97bb894534807",
    "alloc/vec/spec_from_iter_nested.rs": "f3c1a06f43734a65e102b6a107e2fd5adadd7d94d9a226d6ffe1610d1e7e1466",
    "core/alloc/global.rs": "d5eb650234e55742cec9bdff8dad0afe187c71b19dce914e7cd1285649dd78c3",
    "core/iter/adapters/flatten.rs": "78ab8b2f86c7e58288d70aad1b3bdec0df5fa838a1b4543c0b165630866b235e",
    "core/iter/adapters/map.rs": "f1e47647ac44777030c282423e3aefb810629dfedca161cef56cff8ef74a03ad",
    "core/slice/iter.rs": "8f169b9da7f4226497e894409a0535a907a0f0f9660c916fb6b7732cd450345f"
}


def _alloc(name: str, size: int, align: int, source: str) -> dict:
    """Describe a nonempty allocator request; zero-sized Vecs do not allocate."""
    if not name or type(size) is not int or size <= 0 or align <= 0 or align & (align - 1):
        raise ValueError("invalid allocation event")
    return {"op": "alloc", "id": name, "size": size, "align": align, "source": source}


def _free(name: str) -> dict:
    """Describe release of exactly the named live allocation."""
    return {"op": "free", "id": name}


def _provider_repeat(name: str, minimum: int, maximum: int, accel: bool,
                     source: str, reason: str) -> dict:
    """Compress sequential provider lifetimes, subject to state restoration."""
    if not 0 <= minimum <= maximum:
        raise ValueError("invalid repeat count")
    return {"op": "repeat_restoring", "id": name, "min": minimum, "max": maximum,
            "body": [_alloc(name + ".provider", 960, 16 if accel else 8,
                            ZKP + "prg.rs:153-199"), _free(name + ".provider")],
            "source": source, "reason": reason}


def vec_trace(parts: tuple[int, ...], prefix: str, source: str, *,
              element_bytes: int = 1, align: int = 1, initial: int = 0) -> tuple[list[dict], str | None, int]:
    """Emit pinned RawVec growth events; return events, live id and capacity.

    ``initial`` is an exact initial capacity/length, e.g. vec![five words]. Each
    append reserves its whole length once (slice or TrustedLen specialization).
    A growth request allocates first, then frees the old allocation, matching
    this worker's inherited GlobalAlloc::realloc implementation. The caller
    owns the returned allocation and must release it at its actual drop point.
    """
    if (type(initial) is not int or initial < 0 or type(element_bytes) is not int
            or element_bytes <= 0 or any(type(n) is not int or n < 0 for n in parts)):
        raise ValueError("invalid vector schedule")
    events = []
    length = capacity = initial
    live = None
    serial = 0
    if initial:
        live = prefix + ".0"
        events.append(_alloc(live, initial * element_bytes, align, source))
    minimum = 8 if element_bytes == 1 else 4 if element_bytes <= 1024 else 1
    for part in parts:
        required = length + part
        if required > capacity:
            capacity = max(2 * capacity, required, minimum)
            serial += 1
            name = f"{prefix}.{serial}"
            events.append(_alloc(name, capacity * element_bytes, align, source))
            if live is not None:
                events.append(_free(live))
            live = name
        length = required
    return events, live, capacity


def dimensions(row: str, role: str = "registration") -> dict:
    """Return the two frozen rows' public dimensions, without reading a proof."""
    if row not in ROWS or role not in WITNESS_BITS:
        raise ValueError("unsupported row or role")
    tau, grind, gates = ROWS[row]
    delta = 256 - grind
    short, longer = divmod(delta, tau)
    depths = [short + 1] * longer + [short] * (tau - longer)
    ell = WITNESS_BITS[role]
    leaves = sum(1 << depth for depth in depths)
    return {"tau": tau, "delta": delta, "gates": gates, "gate_words": (gates + 63) // 64,
            "depths": depths, "ell": ell, "width": (ell + 10 * max(depths) + 7) // 8,
            "leaves": leaves, "nodes": 2 * leaves - 1}


def profile_trace(row: str, prefix: str = "profile") -> list[dict]:
    """Trace Profile::new; its returned Profile is inline, with no live heap.

    Embedded CRT tables are borrowed. encode_words reserves once for each
    TrustedLen/slice extension. The fixed-array FlatMap<u8;8> collection has an
    exact TrustedLen size hint and allocates once, overlapping the final words
    buffer. Namespace and descriptor hashes are streaming stack operations.
    """
    d = dimensions(row)
    parts = (d["tau"], 4, 4 * d["gates"], 4 * d["gates"], 1024,
             256 * d["gate_words"], 1024)
    events, words, _ = vec_trace(parts, prefix + ".crt.words", ZKP + "crt/codec.rs:39-58",
                                element_bytes=8, align=8, initial=5)
    crt_bytes = prefix + ".crt.bytes"
    events += [_alloc(crt_bytes, 8 * (5 + sum(parts)), 1, BALLOT + "profile.rs:234-237"),
               _free(words), _free(crt_bytes)]
    parameters, dims = prefix + ".descriptor.parameters", prefix + ".descriptor.dimensions"
    events += [_alloc(parameters, 12, 1, BALLOT + "profile.rs:263-274"),
               _alloc(dims, 16, 1, BALLOT + "profile.rs:275-282")]
    # Frame.begin writes tag length, tag, field count; field writes length then bytes.
    appends = (2, len(b"PARL/DESC/v1"), 2) + tuple(n for length in (16, 9, 12, 64, 16, 129) for n in (4, length))
    frame, live, _ = vec_trace(appends, prefix + ".descriptor.frame", BALLOT + "framing.rs:154-249")
    events += frame
    # descriptor_bytes locals drop in reverse order while returned frame remains.
    events += [_free(dims), _free(parameters), _free(live)]
    return events


def backend_trace(row: str, role: str, seats: int = 3, accel: bool = False,
                  prefix: str = "backend") -> list[dict]:
    """Trace proof::verify from codec entry through normal success/rejection.

    The proof, statement and public list are borrowed and owned by the frontend
    trace. Admitted valid-shape inputs all have this allocation schedule except
    the count of balanced internal-node provider calls. Codec rejection stops at
    a prefix, then releases live buffers without allocating. Later algebraic
    values and public list bit patterns do not change the allocation schedule.
    This does not model OOM unwinding, native runtime calls, or compiler stack.
    """
    d = dimensions(row, role)
    if type(accel) is not bool:
        raise ValueError("accel must be boolean")
    if role == "vote" and (type(seats) is not int or not 3 <= seats <= 1000):
        raise ValueError("vote seats must be in 3..=1000")
    tau, delta, gates = d["tau"], d["delta"], d["gates"]
    nodes, leaves, width, ell = d["nodes"], d["leaves"], d["width"], d["ell"]
    events: list[dict] = []

    def alloc(name: str, size: int, align: int, source: str) -> None:
        events.append(_alloc(prefix + "." + name, size, align, source))

    def free(*names: str) -> None:
        events.extend(_free(prefix + "." + name) for name in names)

    def providers(name: str, minimum: int, maximum: int, source: str, reason: str) -> None:
        events.append(_provider_repeat(prefix + "." + name, minimum, maximum, accel, source, reason))

    alloc("decode.hidden", 2 * tau, 2, ZKP + "bavc.rs:1125-1151")
    alloc("decode.marks", nodes, 1, ZKP + "bavc.rs:700-748")
    free("decode.marks", "decode.hidden")

    alloc("layout.trees", 24 * tau, 8, ZKP + "vole.rs:327-409")
    alloc("hidden", 2 * tau, 2, ZKP + "bavc.rs:1125-1151")
    alloc("bavc.marks", nodes, 1, ZKP + "bavc.rs:1067")
    alloc("bavc.keys", nodes * 32, 1, ZKP + "bavc.rs:1083")
    providers("bavc.internal", 0, leaves - 1, ZKP + "bavc.rs:1090-1094",
              "There are L-1 internal nodes; only unmarked nodes expand. Every expand_node Prg is dropped on return before the next node; no retained heap changes inside the loop.")
    alloc("bavc.seeds", leaves * 32, 1, ZKP + "bavc.rs:1096")
    alloc("bavc.com", 64, 1, ZKP + "bavc.rs:1097")
    providers("bavc.leaves", leaves - tau, leaves - tau, ZKP + "bavc.rs:758-801",
              "Exactly one hidden leaf per vector skips commit; the other L-tau leaves each create and drop one Prg; hash state and commitment temporaries are inline.")
    alloc("bavc.offsets", 8 * (tau + 1), 8, ZKP + "bavc.rs:1106-1108")
    alloc("bavc.hidden", 2 * tau, 2, ZKP + "bavc.rs:1111")
    free("bavc.keys", "bavc.marks")

    alloc("selector", 32, 8, ZKP + "vole.rs:1023")
    alloc("columns", delta * width, 1, ZKP + "vole.rs:456-459")
    alloc("shares", 320, 8, ZKP + "vole.rs:1027")
    for instance, depth in enumerate(d["depths"]):
        name = f"convert.{instance}"
        alloc(name + ".stack", depth * width, 1, ZKP + "vole/convert.rs:90")
        alloc(name + ".current", width, 1, ZKP + "vole/convert.rs:91")
        providers(name + ".leaves", (1 << depth) - 1, (1 << depth) - 1,
                  ZKP + "vole/convert.rs:92-99",
                  "Verifier skips leaf0 and expands each remaining N_i-1 leaves with a temporary Prg; hidden index changes the seed slot, not allocation count/order.")
        free(name + ".stack", name + ".current")
    alloc("commitment", 64, 1, ZKP + "vole.rs:1058")
    # Explicit drop(opened): completed Reconstruction fields drop in declaration order.
    free("bavc.offsets", "bavc.com", "bavc.seeds", "bavc.hidden")
    alloc("delta.words", 32, 8, ZKP + "vole.rs:1061")
    alloc("delta", 32, 1, ZKP + "vole.rs:1063")
    alloc("labels.heads", delta * 32, 1, ZKP + "vole.rs:522")
    alloc("labels", gates * 32, 1, ZKP + "vole.rs:532")
    free("labels.heads")
    alloc("gates", 10 * d["gate_words"] * 8, 8, ZKP + "vole.rs:1067")
    alloc("gamma", d["gate_words"] * 8, 8, ZKP + "vole.rs:1083")
    alloc("row", d["gate_words"] * 8, 8, ZKP + "vole.rs:1085")
    alloc("q_masks", 320, 1, ZKP + "vole.rs:543")
    alloc("mask.tree", 32, 8, ZKP + "vole.rs:544")
    alloc("mask.gate", 32, 8, ZKP + "vole.rs:545")
    free("mask.gate", "mask.tree")
    alloc("lifted", 256 * width, 1, ZKP + "vole.rs:570")
    free("columns")
    alloc("q", ell * 32, 1, ZKP + "vole.rs:575")
    free("lifted")
    # q/q_masks are moved with mem::take, no copy or shrink allocation.
    free("row", "gamma", "gates", "labels", "delta.words", "shares", "selector", "layout.trees")

    alloc("challenge1", 264, 1, ZKP + "transcript.rs:234")
    alloc("masks", 320, 8, ZKP + "vole.rs:283")
    alloc("d_tilde.bytes", 128, 1, ZKP + "proof.rs:386-392,749-756")
    alloc("challenge2", 104, 1, ZKP + "transcript.rs:279")
    free("d_tilde.bytes")
    if role == "vote":
        alloc("relation.products", 40 * seats, 8, BALLOT + "vote.rs:339")
    for name, count, size in (("state", 1600, 40), ("theta", 1600, 40),
                              ("parity", 320, 40), ("column", 320, 40), ("source", 1600, 8)):
        alloc("relation." + name, count * size, 8, BALLOT + "keccak_relation.rs:229-242,264-273")
    # Scratch fields drop in declaration order, followed by the earlier products local.
    free(*(f"relation.{name}" for name in ("state", "theta", "parity", "column", "source")))
    if role == "vote":
        free("relation.products")
    coefficients, live, _ = vec_trace((192,), prefix + ".coefficients", ZKP + "proof.rs:770-771", initial=32)
    events += coefficients + [_free(live)]
    free("challenge2", "masks", "challenge1")
    # Returned VOLE Reconstruction fields drop in declaration order.
    free("commitment", "hidden", "delta", "q", "q_masks")
    return events


def trace_assumptions() -> dict:
    """State the allocation-policy, source, layout and coverage obligations."""
    return {
        "scope": "normal cold standalone verification, candidate plus diagnostics-only comparator; allocation failure is a separate worker resource failure",
        "source_revision": RUST_REVISION,
        "rust_allocation_policy": [
            "64-bit usize; non-ZST Vec<T> explicit capacity and vec![T;n] request n*sizeof(T)",
            "TrustedLen map/range/fixed-array FlatMap and slice clone/concat reserve exact known lengths",
            "RawVec grow_amortized chooses max(2*capacity,required,MIN_NON_ZERO_CAP)",
            "Resource worker GlobalAlloc inherits default realloc: allocate/copy/free, never in-place growth",
            "No heap allocation in zeroize, field operations, CRT matrix products, SHA3/SHAKE or selector arithmetic",
            "Completed struct fields drop in declaration order; locals in reverse declaration order; moves do not allocate",
        ],
        "layouts": {"u8_bool": [1, 1], "u16": [2, 2], "u64_usize": [8, 8],
                    "Gf256": [32, 8], "Key_Gf256": [40, 8], "Tree": [24, 8],
                    "aes256_portable_provider": [960, 8], "aes256_accel_provider": [960, 16]},
        "layout_obligation": "Target/compiler-specific sizes and alignments are premises, not Rust ABI promises; provider receipt separately pins eight compiled target/mode observations",
        "provider_receipt": "docs/validation/parliament_ballot_resource_accounting_20261104/provider-layout.json",
        "provider_receipt_sha256": "3483a68bc29109f62a05dd266d10db36e026f63c258bb02ebfca72b55d4ec42c",
        "all_input_classes": [
            "Proof length/counter/padding/grinding/opening rejection follows a prefix of decode events and allocation-free cleanup",
            "Admitted proof codec shape fixes all retained buffer sizes; field byte values are always representable",
            "Public vote-list contents and hidden challenge indices affect arithmetic and provider repetitions, not retained buffer sizes or lifetimes",
            "Every accepted vote list length 3..1000 must be composed/replayed; registration backend has no variable-size allocation",
            "Each repeat_restoring body must restore full allocator free topology and retained live-allocation state, for all repetitions in its finite count interval",
        ],
        "exclusions": ["caller-owned buffers (owned by frontend trace)", "native/TLS allocator activity",
                       "stack, allocator implementation metadata, compiler spills and code/globals",
                       "compiler transformations of source allocation requests unless separately established for the target build",
                       "panic/unwind/abort, concurrency, whole-process and production integration guarantees"],
        "complete_certificate": False,
    }


def source_inventory(repo: Path = ROOT) -> dict:
    """Fail closed on source drift before a consumer promotes a trace result."""
    paths = sorted(path.relative_to(repo).as_posix() for directory in SOURCE_DIRECTORIES
                   for path in (repo / directory).rglob("*.rs"))
    hashes = {path: hashlib.sha256((repo / path).read_bytes()).hexdigest() for path in paths}
    digest = hashlib.sha256(json.dumps(hashes, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    if digest != REVIEWED_SOURCE_SHA256:
        raise ValueError("backend arena trace source drift; review before updating its pin")
    return {"aggregate_sha256": digest, "files": hashes,
            "scope": "reviewed Rust code only; orchestration must additionally bind manifests, lock, dependencies, compiler, allocator worker and binary identity"}


class _RustdocCode(HTMLParser):
    """Extract original Rust from rustdoc's code region, omitting line anchors."""

    def __init__(self) -> None:
        super().__init__()
        self.active = False
        self.anchor = 0
        self.parts: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag == "code":
            self.active = True
        elif tag == "a" and self.active:
            self.anchor += 1

    def handle_endtag(self, tag: str) -> None:
        if tag == "code":
            self.active = False
        elif tag == "a" and self.active:
            self.anchor -= 1

    def handle_data(self, data: str) -> None:
        if self.active and not self.anchor:
            self.parts.append(data)


def rust_source_inventory(rustdoc_root: Path) -> dict:
    """Check exact installed Rust source text against the reviewed revision."""
    hashes = {}
    for path, expected in RUST_SOURCE_SHA256.items():
        parser = _RustdocCode()
        parser.feed((rustdoc_root / (path + ".html")).read_text())
        digest = hashlib.sha256("".join(parser.parts).encode()).hexdigest()
        if digest != expected:
            raise ValueError(f"reviewed Rust allocation source changed: {path}")
        hashes[path] = digest
    return {"revision": RUST_REVISION, "files": hashes}


def main() -> int:
    """Print the guarded trace and explicit assumptions; make no file changes."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--row", choices=ROWS, default="candidate")
    parser.add_argument("--role", choices=WITNESS_BITS, default="vote")
    parser.add_argument("--seats", type=int, default=1000)
    parser.add_argument("--accel", action="store_true")
    parser.add_argument("--rustdoc-root", type=Path, help="optional Rust1.99.1 html/src directory to verify")
    args = parser.parse_args()
    data = {"assumptions": trace_assumptions(), "sources": source_inventory(),
            "profile": profile_trace(args.row),
            "backend": backend_trace(args.row, args.role, args.seats, args.accel)}
    if args.rustdoc_root is not None:
        data["rust_allocation_sources"] = rust_source_inventory(args.rustdoc_root)
    print(json.dumps(data, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
