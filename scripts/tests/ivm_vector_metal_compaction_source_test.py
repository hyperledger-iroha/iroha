#!/usr/bin/env python3
"""Guard current Metal custody, accepted completion and deterministic fallback.

This source guard follows the canonical owners; Rust tests verify execution.
"""
from __future__ import annotations

from pathlib import Path
import re
import unittest

from scripts.formal.rust_text import mask_rust_comments

ROOT = Path(__file__).resolve().parents[2]
OWNER_PATHS = {
    "vector": "crates/ivm/src/vector.rs",
    "buffers": "crates/ivm/src/vector/metal_buffers.rs",
    "runtime": "crates/ivm/src/vector/metal_runtime.rs",
    "aes": "crates/ivm/src/vector/metal_aes.rs",
    "merkle": "crates/ivm/src/vector/metal_merkle.rs",
}


class GuardError(AssertionError):
    """A current Metal ownership or acceptance invariant is absent."""


def _require(condition: bool, message: str) -> None:
    if not condition:
        raise GuardError(message)


def _read_sources() -> dict[str, str]:
    sources = {}
    for owner, relative in OWNER_PATHS.items():
        path = ROOT / relative
        _require(path.is_file() and not path.is_symlink(), "invalid Metal owner: " + relative)
        _require(path.resolve().is_relative_to(ROOT), "Metal owner escapes repository")
        sources[owner] = path.read_text(encoding="utf-8")
    return sources


def _function(source: str, name: str) -> str:
    """Read the first actual function body, excluding comments and strings."""
    masked = mask_rust_comments(source)
    match = re.search(r"\bfn\s+" + re.escape(name) + r"(?:<[^\n{]*>)?\s*\(", masked)
    _require(match is not None, "missing Metal function: " + name)
    opening = masked.find("{", match.end())
    _require(opening >= 0, "missing function body: " + name)
    end, depth = opening + 1, 1
    while end < len(masked) and depth:
        depth += (masked[end] == "{") - (masked[end] == "}")
        end += 1
    _require(depth == 0, "unterminated function body: " + name)
    return source[match.start():end]


def _compact(source: str) -> str:
    return "".join(source.split())


def _require_order(source: str, anchors: tuple[str, ...], label: str) -> None:
    compact = _compact(source)
    cursor = 0
    for anchor in anchors:
        position = compact.find(_compact(anchor), cursor)
        _require(position >= 0, label + " lacks ordered invariant: " + anchor)
        cursor = position + len(_compact(anchor))


def _require_fallible_call(source: str, name: str, label: str) -> None:
    """Require the sole real call to propagate refusal before reading output."""
    masked = mask_rust_comments(source)
    matches = list(re.finditer(r"\b" + re.escape(name) + r"\s*\(", masked))
    _require(len(matches) == 1, label + " lacks one actual call: " + name)
    end, depth = matches[0].end(), 1
    while end < len(masked) and depth:
        depth += (masked[end] == "(") - (masked[end] == ")")
        end += 1
    _require(depth == 0 and masked[end:].lstrip().startswith("?;"),
             label + " must propagate refusal: " + name)


def _validate_sources(sources: dict[str, str]) -> None:
    """Require real funded backing, physical completion and public GPU geometry."""
    _require(set(sources) == set(OWNER_PATHS), "Metal owner inventory differs")
    vector, buffers, runtime, aes, merkle = (
        sources[name] for name in ("vector", "buffers", "runtime", "aes", "merkle")
    )
    for module in ("metal_buffers", "metal_runtime"):
        _require(vector.count("mod " + module + ";") == 1, "Metal module edge differs: " + module)
    _require(vector.count('#[path = "vector/metal_aes.rs"]\nmod metal_aes;') == 1,
             "AES source owner edge differs")
    implementations = re.findall(r"^impl MetalBufferElement for (\w+) \{\}$", vector, re.MULTILINE)
    _require(implementations == ["u8", "u32", "u64"], "Metal input elements must be padding-free primitives")
    _require_order(_function(vector, "metal_input_buffer"), (
        "if byte_len > core::mem::size_of_val(values)", "return None;",
        "MetalBuffer::allocate(device, byte_len)?", "buffer.copy_input(values, byte_len)?",
    ), "typed inclusive input bound")
    _require("MetalBuffer::allocate(device, byte_len)" in _function(vector, "metal_output_buffer"),
             "output lost its funded native owner")
    _require_order(_function(buffers, "allocate"), (
        "health.identity() != device.registryID()", "return None;",
        "owner().try_unified_buffer(len, alignment).ok()?",
        "device.newBufferWithBytesNoCopy_length_options_deallocator(",
        "backing.as_ptr().cast()", "backing.capacity()",
    ), "physical buffer custody")
    _require_order(_function(buffers, "copy_input"), (
        "len > std::mem::size_of_val(values)", "len > self.backing.len()",
        "self.claimed.get()", "self.pending.get()", "return None;",
    ), "initialized input copy")
    dispatch = _function(vector, "metal_dispatch")
    _require_order(dispatch, (
        "if !metal_runtime_allowed()", "return None;",
        "metal_buffers::Command::prepare(queue, buffers)?", "command.encode(",
        "command.quarantine()", "command.commit()", "finalize_command_buffer(&mut command, context)",
        "matches!(outcome, MetalCommandOutcome::Complete)",
        "metal_dispatch_result_allowed(completed, command.usable() && metal_runtime_allowed())",
        "metal_receipts::record_completion(receipt, accepted)", "accepted.then_some(())",
    ), "accepted native dispatch")
    _require("completed&&backend_allowed" in _compact(_function(vector, "metal_dispatch_result_allowed")),
             "failed or quarantined completion became acceptable")
    _require_order(_function(buffers, "prepare"), (
        "health.identity() != queue.device().registryID()",
        "!iroha_allocation::ChargedShared::ptr_eq(&health, &buffer.health)",
        "return None;", "PreparedClaims::acquire(buffers)?", "owner().try_native_command()?",
        "queue.commandBuffer()?",
    ), "one physical command owner")
    _require_order(_function(buffers, "commit"), (
        "self.phase != CommandPhase::Prepared", "!self.usable()", "return false;",
        "self.phase = CommandPhase::Pending", "buffer.pending.set(true)", "self.native.commit()",
    ), "native enqueue custody")
    _require_order(_function(buffers, "observe_completion"), (
        "self.phase != CommandPhase::Pending", "return None;", "self.native.status()",
        "MTLCommandBufferStatus::Completed => true", "MTLCommandBufferStatus::Error => false",
        "_ => return None", "self.phase = CommandPhase::Complete", "buffer.pending.set(false)",
    ), "actual terminal completion")
    _require_order(_function(buffers, "mark_uncertain"), (
        "self.phase == CommandPhase::Pending", "self.phase = CommandPhase::Uncertain",
        "self.health.quarantine(true)",
    ), "irreversibly uncertain work")
    _require_order(_function(vector, "finalize_command_buffer"), (
        "command.observe_completion()", "MetalCommandOutcome::Complete", "command.quarantine()",
        "MetalCommandOutcome::Failed", "command.mark_uncertain()", "MetalCommandOutcome::Uncertain",
    ), "bounded completion observation")
    _require("static DEVICES: DeviceRegistry<MetalState>" in runtime, "physical registry owner missing")
    _require_order(_function(runtime, "with_state"), (
        "if !metal_runtime_allowed()", "return None;", "discover()", "record_allowed(&lease)",
        "lease.value()", "bind(&lease, || Some(call(state)))",
    ), "physical execution lease")
    _require_order(_function(vector, "bundled_metal_library"), (
        "!= METAL_KERNELS_SHA256", "return None;", "DispatchData::from_static_bytes(METAL_KERNELS)",
        "device.newLibraryWithData_error(&data).ok()",
    ), "authenticated embedded kernel")
    _require("transfer_bytes>=MIN_GPU_TRANSFER_BYTES" in _compact(_function(vector, "gpu_launch_eligible")),
             "GPU eligibility lost public geometry")
    for operation in ("vadd32", "vadd64", "vand", "vxor", "vor"):
        body = _function(vector, operation)
        _require(body.count("gpu_launch_eligible(FIXED_VECTOR_TRANSFER_BYTES)") == 2,
                 "fixed operation lost public workload gate: " + operation)
        _require_order(body, ("metal_" + operation + "(a, b)", "crate::cuda::" + operation + "_cuda_into", operation + "_slice"),
                       "deterministic hardware fallback " + operation)
        _require("target_arch" not in body, "fixed fallback duplicated by architecture")
    _require("Some(MetalKernel::Add64)" in _function(vector, "metal_vadd64"), "production completion receipt missing")
    _require_order(_function(aes, "attempt"), (
        "u32::try_from(keys.len()).ok()?", "NSUInteger::try_from(states.len()).ok()?",
        "states.len().checked_mul(16)?", "with_metal_state_try(",
        "metal_output_buffer(&ctx.device, output_bytes)?", "metal_dispatch(",
    ), "bounded AES batch")
    _require("blocks != destination.len() || !buffer.usable()" in _function(aes, "copy_into"),
             "AES publishes a foreign, failed or unbounded output")
    _require(vector.count('#[path = "vector/metal_merkle.rs"]\nmod metal_merkle;') == 1,
             "Merkle source owner edge differs")
    _require_order(_function(runtime, "metal_root_from_bytes_auto"), (
        "select(&MERKLE_ROOT_PROGRESS", "run(|| metal_merkle::root_from_bytes(data, chunk))",
    ), "selected canonical Merkle producer")
    _require_order(_function(merkle, "root_from_bytes"), (
        "metal_runtime::current_selection()?", "chunk_leaves(data, chunk)?",
        "metal_merkle_root(&digests)?", "drop(digests)",
        "Readback {", "selection,", "value: root,", ".publish()",
    ), "canonical root readback owner")
    _require("reduce(digests,true)" in _compact(_function(merkle, "metal_merkle_root")),
             "canonical Merkle producer lost its domain policy")
    _require("reduce(digests,false)" in _compact(_function(merkle, "metal_sha256_pairs_reduce")),
             "raw SHA pair producer gained canonical Merkle markers")
    reduction = _function(merkle, "reduce")
    attempt = _function(merkle, "reduce_attempt")
    _require("reduce_attempt(digests,canonical_merkle)?.publish()" in _compact(reduction),
             "Merkle reduction bypassed accepted readback")
    _require(reduction.count("if canonical_merkle {") == 1
             and attempt.count("if canonical_merkle {") == 2,
             "canonical Merkle marker sites differ")
    _require(reduction.count("root[31] |= 1;") == 1
             and attempt.count("node[31] |= 1;") == 1
             and attempt.count("root[31] |= 1;") == 1,
             "canonical Merkle domain markers missing")
    _require_fallible_call(attempt, "metal_dispatch", "accepted Merkle completion")
    _require_order(attempt, (
        "metal_runtime::current_selection()?", "metal_dispatch(",
        "Some(MetalKernel::Sha256Pairs)", "std::slice::from_raw_parts(",
        "root.copy_from_slice(&cur[..32])", "Some(Readback {", "selection,", "value: root,",
    ), "completed Merkle bytes retain original physical owner")
    _require("self.selection.run(||self.value)" in _compact(_function(merkle, "publish")),
             "Merkle readback bypassed original owner acceptance")
    _require_order(_function(runtime, "run"), (
        "!metal_runtime_allowed()", "!record_allowed(&self.lease)",
        "self.lease.value().is_none()", "return None;", "bind(&self.lease, || Some(call()))",
    ), "original physical owner acceptance")
    for retired in ("newLibraryWithSource_options_error", "bit_pipe_compile_count"):
        _require(retired not in vector, "retired Metal implementation returned")


class IvmVectorMetalCompactionSourceTest(unittest.TestCase):
    """Current owners and representative fail-closed source mutations."""

    def setUp(self) -> None:
        self.sources = _read_sources()

    def test_repository_contract(self) -> None:
        _validate_sources(self.sources)

    def test_safety_ownership_and_acceptance_mutations_are_rejected(self) -> None:
        _validate_sources(self.sources)
        mutations = (
            ("vector", "if byte_len > core::mem::size_of_val(values)", "if byte_len >= core::mem::size_of_val(values)"),
            ("vector", "impl MetalBufferElement for u64 {}", "impl MetalBufferElement for PaddedWord {}"),
            ("vector", "completed && backend_allowed", "completed || backend_allowed"),
            ("vector", "record_completion(receipt, accepted)", "record_completion(receipt, completed)"),
            ("vector", "command.usable() && metal_runtime_allowed()", "metal_runtime_allowed()"),
            ("vector", "Some(MetalKernel::Add64)", "None"),
            ("vector", "!= METAL_KERNELS_SHA256", "== METAL_KERNELS_SHA256"),
            ("vector", "if gpu_launch_eligible(FIXED_VECTOR_TRANSFER_BYTES)", "if true"),
            ("vector", "transfer_bytes >= MIN_GPU_TRANSFER_BYTES", "transfer_bytes < MIN_GPU_TRANSFER_BYTES"),
            ("vector", "crate::cuda::vadd32_cuda_into", "crate::cuda::wrong_order"),
            ("merkle", "node[31] |= 1;", "node[31] |= 0;"),
            ("buffers", "buffer.pending.set(true)", "buffer.pending.set(false)"),
            ("buffers", "MTLCommandBufferStatus::Completed => true", "MTLCommandBufferStatus::Completed => false"),
            ("buffers", "self.health.quarantine(true)", "self.health.quarantine(false)"),
            ("buffers", "!iroha_allocation::ChargedShared::ptr_eq(&health, &buffer.health)", "false"),
            ("aes", "u32::try_from(keys.len()).ok()?", "keys.len() as u32"),
            ("aes", "blocks != destination.len() || !buffer.usable()", "blocks != destination.len()"),
            ("runtime", "record_allowed(&lease)", "true"),
        )
        for owner, old, new in mutations:
            with self.subTest(owner=owner, marker=old):
                self.assertIn(old, self.sources[owner])
                changed = dict(self.sources)
                changed[owner] = changed[owner].replace(old, new, 1)
                with self.assertRaises(GuardError):
                    _validate_sources(changed)

    def test_merkle_readback_call_edges_are_rejected(self) -> None:
        _validate_sources(self.sources)
        changed = dict(self.sources)
        changed["vector"] = changed["vector"].replace(
            'mod metal_merkle;', 'mod disconnected_merkle;', 1
        )
        with self.assertRaisesRegex(GuardError, "Merkle source owner edge"):
            _validate_sources(changed)
        mutations = (
            ("runtime", "metal_root_from_bytes_auto", "metal_merkle::root_from_bytes(data, chunk)",
             "metal_merkle::disconnected_root(data, chunk)"),
            ("merkle", "root_from_bytes", "metal_merkle_root(&digests)?",
             "metal_sha256_pairs_reduce(&digests)?"),
            ("merkle", "metal_merkle_root", "reduce(digests, true)", "reduce(digests, false)"),
            ("merkle", "metal_sha256_pairs_reduce", "reduce(digests, false)", "reduce(digests, true)"),
            ("merkle", "reduce", "root[31] |= 1;", "root[31] |= 0;"),
            ("merkle", "reduce_attempt", "root[31] |= 1;", "root[31] |= 0;"),
            ("merkle", "reduce", "reduce_attempt(digests, canonical_merkle)?.publish()",
             "Some(reduce_attempt(digests, canonical_merkle)?.value)"),
            ("merkle", "reduce_attempt", "let selection = metal_runtime::current_selection()?;",
             "let selection = foreign_selection()?;"),
            ("merkle", "reduce_attempt", "Some(MetalKernel::Sha256Pairs)", "None"),
            ("merkle", "reduce_attempt", 'Some(MetalKernel::Sha256Pairs),\n                )?;',
             'Some(MetalKernel::Sha256Pairs),\n                );'),
            ("merkle", "publish", "self.selection.run(|| self.value)", "Some(self.value)"),
            ("runtime", "run", "!record_allowed(&self.lease)", "false"),
        )
        for owner, function, old, new in mutations:
            with self.subTest(owner=owner, function=function, marker=old):
                body = _function(self.sources[owner], function)
                self.assertEqual(body.count(old), 1)
                changed = dict(self.sources)
                changed[owner] = changed[owner].replace(body, body.replace(old, new, 1), 1)
                with self.assertRaises(GuardError):
                    _validate_sources(changed)
        with self.assertRaisesRegex(GuardError, "owner inventory"):
            _validate_sources({name: text for name, text in self.sources.items() if name != "merkle"})

    def test_missing_owner_is_rejected(self) -> None:
        with self.assertRaisesRegex(GuardError, "owner inventory"):
            _validate_sources({name: text for name, text in self.sources.items() if name != "buffers"})

    def test_documentation_can_evolve(self) -> None:
        changed = dict(self.sources)
        changed["vector"] = "// Current owner documentation.\n" + changed["vector"]
        _validate_sources(changed)


if __name__ == "__main__":
    unittest.main()
