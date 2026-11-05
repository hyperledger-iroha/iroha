"""Independent ARM64 instruction fixtures for conservative stack-region analysis."""

import importlib.util
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[1] / "parliament_ballot_stack_audit.py"
SPEC = importlib.util.spec_from_file_location("ballot_stack_audit", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


def function(lines, extra_starts=(), stubs=None):
    """Place a small actual instruction sequence at a fixed ARM64 address."""
    instructions = {0x1000 + 4 * i: (line.split(" ", 1)[0], line.partition(" ")[2])
                    for i, line in enumerate(lines)}
    starts = {0x1000: "root", **dict(extra_starts)}
    return MODULE.analyze_function(0x1000, 0x1000 + 4 * len(lines), instructions,
                                   starts, stubs or {}, "root")


def test_actual_reservation_prologue_and_multiple_epilogues():
    # Exact portable Reservation::verify prologue: 96-byte paired save plus
    # 1936-byte local frame. Two ordinary exits restore both pieces.
    result = function([
        "stp x28, x27, [sp, #-0x60]!", "stp x26, x25, [sp, #0x10]",
        "stp x24, x23, [sp, #0x20]", "stp x22, x21, [sp, #0x30]",
        "stp x20, x19, [sp, #0x40]", "stp x29, x30, [sp, #0x50]",
        "add x29, sp, #0x50", "sub sp, sp, #0x790",
        "cbz x0, 0x1034 <second>", "add sp, sp, #0x790",
        "ldp x29, x30, [sp, #0x50]", "ldp x28, x27, [sp], #0x60", "ret",
        "add sp, sp, #0x790", "ldp x29, x30, [sp, #0x50]",
        "ldp x28, x27, [sp], #0x60", "ret",
    ])
    assert result["maximum_sp_decrement_bytes"] == 2032
    assert result["local_frame_plus_red_zone_bytes"] == 2160
    assert not result["frame_errors"] and not result["gaps"]
    assert result["visited_instructions"] == 17


def test_worker_prologue_and_shifted_immediate():
    result = function(["sub sp, sp, #0x150", "sub sp, sp, #0x1, lsl #12",
                       "add sp, sp, #0x1, lsl #0xc", "add sp, sp, #0x150", "ret"])
    assert result["maximum_sp_decrement_bytes"] == 4432
    assert not result["frame_errors"]


@pytest.mark.parametrize("store", ["stlur x9, [x8, #0x8]", "stlur w9, [x8, #-0x100]",
                                  "stlur xzr, [x8, #255]", "stlur wzr, [sp]",
                                  "stlur x29, [sp, #-8]"])
def test_stlur_preserves_sp_fp_and_continues_after_control_publication(store):
    result = function(["sub sp, sp, #16", "mov x29, sp", store,
                       "sub sp, sp, #32", "mov sp, x29", "add sp, sp, #16", "ret"])
    assert result["maximum_sp_decrement_bytes"] == 48
    assert result["visited_instructions"] == 7
    assert not result["frame_errors"] and not result["gaps"]


@pytest.mark.parametrize("store", ["stlur x0, [sp, #-256]", "stlur x0, [sp], #16",
                                  "stlur x0, [sp, #-16]!", "stlur x0, [x8, #256]",
                                  "stlur x0, [x8, #-257]", "stlur x0, [x8, x1]",
                                  "stlur sp, [x8]", "stlur x31, [x8]"])
def test_stlur_does_not_bypass_operand_or_red_zone_checks(store):
    result = function([store, "ret"])
    assert result["frame_errors"][0]["kind"] == "unsupported_stack_effect"
    assert MODULE.call_region({0x1000: result}, 0x1000)["complete_stack_bound_bytes"] is None


def test_actual_fatal_publication_keeps_exit_and_fallthrough_unknown():
    # Both branches of the new fatal::exhausted body, with addresses relocated.
    result = function([
        "adrp x8, 0x7000", "add x8, x8, #0xd58", "ldapr x8, [x8]",
        "cbz x8, 0x102c <invalid>", "ldapur x9, [x8, #0x8]", "cmp x9, #2",
        "b.ne 0x102c <invalid>", "mov w9, #3", "stlur x9, [x8, #0x8]",
        "mov w0, #0x49", "b 0x1030 <exit>", "mov w0, #0x4a",
        "stp x29, x30, [sp, #-0x10]!", "mov x29, sp", "bl 0x9000 <_exit>",
    ], stubs={0x9000: "__exit"})
    assert result["maximum_sp_decrement_bytes"] == 16
    assert result["visited_instructions"] == 15
    assert not result["frame_errors"]
    region = MODULE.call_region({0x1000: result}, 0x1000)
    assert region["frontier_kind_counts"] == {"function_fallthrough": 1, "native_edge": 1}
    assert region["complete_stack_bound_bytes"] is None


@pytest.mark.parametrize("instruction", ["eon x10, x12, x10", "eon x11, x12, x11",
                                        "eon w0, wzr, w29", "eon xzr, x29, x30",
                                        "eon w0, w1, w2, lsl #31", "eon x0, x1, x2, lsr #63",
                                        "eon x0, x1, x2, asr #0x3f", "eon w0, w1, w2, ror #0"])
def test_eon_store_free_register_operation_preserves_unwritten_sp_fp(instruction):
    result = function(["sub sp, sp, #16", "mov x29, sp", instruction,
                       "sub sp, sp, #32", "mov sp, x29", "add sp, sp, #16", "ret"])
    assert result["maximum_sp_decrement_bytes"] == 48
    assert result["visited_instructions"] == 7
    assert not result["frame_errors"] and not result["gaps"]


@pytest.mark.parametrize("instruction", ["eon x29, x0, x1", "eon w29, w0, w1"])
def test_eon_fp_destination_cannot_justify_later_stack_restore(instruction):
    result = function(["sub sp, sp, #16", "mov x29, sp", instruction, "mov sp, x29", "ret"])
    assert result["frame_errors"][0]["kind"] == "unsupported_stack_effect"
    assert MODULE.call_region({0x1000: result}, 0x1000)["complete_stack_bound_bytes"] is None


@pytest.mark.parametrize("instruction", ["eon sp, x0, x1", "eon wsp, w0, w1",
                                        "eon x0, sp, x1", "eon x0, x1, w2",
                                        "eon x31, x0, x1", "eon x0, x1, #1",
                                        "eon w0, w1, w2, lsl #32", "eon x0, x1, x2, lsr #64",
                                        "eon x0, x1, x2, asr #-1", "eon x0, x1, x2, future #1",
                                        "eon.future x0, x1, x2", "stlur.future x0, [x1]"])
def test_new_scalar_opcodes_reject_unknown_forms_and_stack_pointer_operands(instruction):
    result = function([instruction, "ret"])
    assert result["frame_errors"][0]["kind"] == "unsupported_stack_effect"
    assert MODULE.call_region({0x1000: result}, 0x1000)["complete_stack_bound_bytes"] is None


@pytest.mark.parametrize("instruction", [
    "ldursb w8, [x29, #-0xa8]", "ldursb x30, [sp, #255]", "ldursb wzr, [sp]",
    "ldursb xzr, [x0, #-256]", "orn w13, w13, w11", "orn xzr, x29, x30",
    "orn w0, wzr, w29, lsl #31", "orn x0, x1, x2, lsr #63",
    "orn x0, x1, x2, asr #0", "orn w0, w1, w2, ror #0x1f",
    "umaxv.16b b0, v0", "umaxv.8b b31, v31", "umaxv.4h h0, v1",
    "umaxv.8h h29, v29", "umaxv.4s s31, v0", "mvni.8h v4, #0xf",
    "mvni.4h v31, #255, lsl #8", "mvni.2s v29, #0, lsl #24",
    "mvni.4s v0, #255, msl #8", "mvni.4s v31, #0xff, msl #16",
    "rax1.2d v14, v13, v10", "rax1.2d v29, v30, v31",
])
def test_current_image_opcodes_preserve_integer_sp_and_unwritten_fp(instruction):
    # Reserve below the known FP before the actual signed-byte load. SIMD
    # register 29 is a different register bank and must not clobber integer FP.
    result = function(["sub sp, sp, #64", "mov x29, sp", "sub sp, sp, #64", instruction,
                       "sub sp, sp, #32", "mov sp, x29", "add sp, sp, #64", "ret"])
    assert result["maximum_sp_decrement_bytes"] == 160
    assert result["visited_instructions"] == 8
    assert not result["frame_errors"] and not result["gaps"]


@pytest.mark.parametrize("instruction", [
    "ldursb x29, [sp]", "ldursb w29, [x29, #-8]",
    "orn x29, x0, x1", "orn w29, w0, w1",
])
def test_current_scalar_fp_writes_refuse_a_stale_fp_stack_restore(instruction):
    result = function(["sub sp, sp, #16", "mov x29, sp", instruction, "mov sp, x29", "ret"])
    assert result["frame_errors"][0]["kind"] == "unsupported_stack_effect"
    assert result["frame_errors"][0]["address"] == "0x100c"
    assert result["frame_errors"][0]["detail"] == "mov sp, x29: unsupported SP write"
    assert MODULE.call_region({0x1000: result}, 0x1000)["complete_stack_bound_bytes"] is None


@pytest.mark.parametrize("instruction", [
    "ldursb sp, [x0]", "ldursb w31, [x0]", "ldursb w0, [w1]",
    "ldursb w0, [x31]", "ldursb w0, [xzr]", "ldursb w0, [x1, #256]",
    "ldursb w0, [x1, #-257]", "ldursb w0, [x1, x2]",
    "ldursb w0, [sp, #-16]!", "ldursb w0, [sp], #16",
    "ldursb w0, [x29], #16", "ldursb w0, [sp, #-129]",
    "ldursb.future w0, [x1]", "orn sp, x0, x1", "orn x0, sp, x1",
    "orn x0, x1, w2", "orn x31, x0, x1", "orn w0, w1, w2, lsl #32",
    "orn x0, x1, x2, ror #64", "orn x0, x1, x2, lsr #-1",
    "orn x0, x1, x2, future #1", "orn.16b v0, v1, v2",
    "umaxv b0, v0", "umaxv.2s s0, v1", "umaxv.2d d0, v1",
    "umaxv.16b h0, v1", "umaxv.16b w0, v1", "umaxv.16b b32, v0",
    "umaxv.16b b0, v32", "umaxv.16b b0, sp", "umaxv.future b0, v0",
    "mvni v0, #15", "mvni.16b v0, #15", "mvni.2d v0, #15",
    "mvni.8h sp, #15", "mvni.8h x29, #15", "mvni.8h v32, #15",
    "mvni.8h v0, #256", "mvni.8h v0, #-1", "mvni.8h v0, #15, lsl #16",
    "mvni.4s v0, #15, lsl #32", "mvni.8h v0, #15, msl #8",
    "mvni.4s v0, #15, msl #0", "mvni.4s v0, #15, msl #24",
    "mvni.4s v0, #15, lsr #8", "mvni.future v0, #15",
    "rax1 v0, v1, v2", "rax1.4s v0, v1, v2", "rax1.2d x29, v1, v2",
    "rax1.2d v0, sp, v2", "rax1.2d v0, v1, v32", "rax1.2d v0, v1, v2, #1",
])
def test_current_image_opcodes_reject_unreviewed_operand_forms(instruction):
    result = function([instruction, "ret"])
    assert result["frame_errors"][0]["kind"] == "unsupported_stack_effect"
    assert MODULE.call_region({0x1000: result}, 0x1000)["complete_stack_bound_bytes"] is None


@pytest.mark.parametrize("instruction", [
    "bcax.16b v17, v30, v20, v21", "bcax.16b v29, v0, v31, v29",
    "bit.16b v0, v5, v21", "bit.8b v29, v30, v31",
    "umov.h w2, v0[0]", "umov.h wzr, v31[7]",
    "umov.b w30, v29[15]", "umov.b wzr, v31[0]",
    "umov.s w0, v29[3]", "umov.s wzr, v31[0]",
    "umov.d x30, v29[1]", "umov.d xzr, v31[0]",
])
def test_remaining_image_opcodes_preserve_sp_and_unwritten_integer_fp(instruction):
    result = function(["sub sp, sp, #16", "mov x29, sp", instruction,
                       "sub sp, sp, #32", "mov sp, x29", "add sp, sp, #16", "ret"])
    assert result["maximum_sp_decrement_bytes"] == 48
    assert result["visited_instructions"] == 7
    assert not result["frame_errors"] and not result["gaps"]


@pytest.mark.parametrize("instruction", [
    "umov.b w29, v0[15]", "umov.h w29, v29[7]",
    "umov.s w29, v1[3]", "umov.d x29, v31[1]",
])
def test_umov_integer_fp_destination_prevents_stale_fp_stack_restore(instruction):
    result = function(["sub sp, sp, #16", "mov x29, sp", instruction, "mov sp, x29", "ret"])
    assert result["frame_errors"] == [{
        "address": "0x100c", "kind": "unsupported_stack_effect",
        "detail": "mov sp, x29: unsupported SP write", "caller_depth_bytes": 16,
    }]
    assert MODULE.call_region({0x1000: result}, 0x1000)["complete_stack_bound_bytes"] is None


@pytest.mark.parametrize("instruction", [
    "bcax v0, v1, v2, v3", "bcax.8b v0, v1, v2, v3",
    "bcax.2d v0, v1, v2, v3", "bcax.future v0, v1, v2, v3",
    "bcax.16b v0, v1, v2", "bcax.16b v0, v1, v2, v3, v4",
    "bcax.16b sp, v1, v2, v3", "bcax.16b x29, v1, v2, v3",
    "bcax.16b v0, v1, v2, sp", "bcax.16b v0, v1, v2, v32",
    "bcax.16b v0, v1, v2, [sp], #16",
    "bit v0, v1, v2", "bit.4s v0, v1, v2", "bit.future v0, v1, v2",
    "bit.16b sp, v1, v2", "bit.8b x29, v1, v2", "bit.16b v0, sp, v2",
    "bit.16b v0, v32, v2", "bit.8b v0, v1", "bit.16b v0, v1, v2, v3",
    "bit.16b v0, v1, [x29], #16",
    "umov w0, v0[0]", "umov.future w0, v0[0]", "umov.8h w0, v0[0]",
    "umov.h sp, v0[0]", "umov.h wsp, v0[0]", "umov.d sp, v0[0]",
    "umov.h x0, v0[0]", "umov.d w0, v0[0]", "umov.h w31, v0[0]",
    "umov.d x31, v0[0]", "umov.h w0, v32[0]", "umov.h w0, x29[0]",
    "umov.h w0, sp[0]", "umov.h w0, [sp], #16", "umov.h w0, [x29], #16",
    "umov.b w0, v0[16]", "umov.h w0, v0[8]", "umov.s w0, v0[4]",
    "umov.d x0, v0[2]", "umov.h w0, v0[-1]", "umov.h w0, v0[0x1]",
    "umov.h w0, v0[x1]", "umov.h w0, v0", "umov.h w0, v0[0]!",
    "umov.h w0, v0[0], #1",
])
def test_remaining_image_opcodes_reject_unreviewed_forms(instruction):
    result = function([instruction, "ret"])
    assert result["frame_errors"][0]["kind"] == "unsupported_stack_effect"
    assert MODULE.call_region({0x1000: result}, 0x1000)["complete_stack_bound_bytes"] is None


def test_fp_restore_and_unknown_fp_fail_closed():
    valid = function(["stp x29, x30, [sp, #-0x10]!", "mov x29, sp",
                      "sub sp, sp, #0x40", "mov sp, x29", "ldp x29, x30, [sp], #0x10", "ret"])
    assert valid["maximum_sp_decrement_bytes"] == 80
    assert not valid["frame_errors"]
    invalid = function(["mov x29, x0", "mov sp, x29", "ret"])
    assert invalid["frame_errors"][0]["kind"] == "unsupported_stack_effect"


@pytest.mark.parametrize("clobber", ["ldadd x0, x29, [x1]", "swpab w0, w29, [x1]",
                                    "ldp w28, w29, [sp]", "ldr x0, [x29], #16"])
def test_secondary_fp_destinations_cannot_keep_a_stale_known_offset(clobber):
    result = function(["sub sp, sp, #16", "mov x29, sp", clobber, "mov sp, x29", "ret"])
    assert any(e["kind"] == "unsupported_stack_effect" for e in result["frame_errors"])


@pytest.mark.parametrize("instruction", ["sub sp, sp, x0", "mov sp, x9", "add wsp, wsp, #16",
                                          "ld1 { v0.16b }, [sp], x0", "unknown x0, x1", "b.future 0x1004"])
def test_unsupported_instructions_and_stack_writes_are_not_zero_cost(instruction):
    result = function([instruction, "ret"])
    assert result["frame_errors"]
    region = MODULE.call_region({0x1000: result}, 0x1000)
    assert not region["complete_ordinary_call_graph"]
    assert region["complete_stack_bound_bytes"] is None


def test_unproved_probe_loop_is_rejected_not_capped():
    result = function(["sub sp, sp, #0x1, lsl #12", "cbnz x0, 0x1000 <loop>", "ret"])
    assert {e["kind"] for e in result["frame_errors"]} == {"inconsistent_sp_merge", "unbalanced_return"}
    assert MODULE.call_region({0x1000: result}, 0x1000)["complete_stack_bound_bytes"] is None


def test_conditional_branch_explores_larger_frame_and_leaf_red_zone():
    result = function(["cbz x0, 0x1010 <larger>", "sub sp, sp, #16", "add sp, sp, #16", "ret",
                       "sub sp, sp, #64", "add sp, sp, #64", "ret"])
    assert result["local_frame_plus_red_zone_bytes"] == 192
    leaf = function(["str x0, [sp, #-8]", "ret"])
    assert leaf["maximum_sp_decrement_bytes"] == 0
    assert leaf["local_frame_plus_red_zone_bytes"] == 128


@pytest.mark.parametrize("access", ["str x0, [sp, #-256]", "ldr x0, [sp, x1]",
                                   "str x0, [x29, #-256]"])
def test_direct_stack_access_cannot_silently_exceed_frame_or_red_zone(access):
    result = function(["mov x29, sp", access, "ret"])
    assert result["frame_errors"][0]["kind"] == "unsupported_stack_effect"


def test_missing_target_instruction_and_unbalanced_return():
    result = function(["b 0x9000 <missing>"])
    assert result["frame_errors"][0]["kind"] == "missing_target"
    instructions = {0x1000: ("b", "0x1004 <hole>"), 0x1008: ("ret", "")}
    result = MODULE.analyze_function(0x1000, 0x100c, instructions, {0x1000: "root"}, {}, "root")
    assert result["frame_errors"][0]["kind"] == "missing_instruction"
    result = function(["sub sp, sp, #16", "ret"])
    assert result["frame_errors"][0]["kind"] == "unbalanced_return"
    for instruction in ["sub sp, sp, #8", "add sp, sp, #16"]:
        result = function([instruction, "ret"])
        assert result["frame_errors"][0]["kind"] == "invalid_sp_state"


def image(blocks, stubs=None):
    """Build separate functions with independent explicit address ranges."""
    starts = {address: name for address, name, _ in blocks}
    instructions = {address + 4 * i: (line.split(" ", 1)[0], line.partition(" ")[2])
                    for address, _, lines in blocks for i, line in enumerate(lines)}
    return {address: MODULE.analyze_function(address, address + 4 * len(lines), instructions,
                                             starts, stubs or {}, name)
            for address, name, lines in blocks}


def test_direct_call_weight_tail_reuse_and_leaf_red_zone_not_repeated():
    functions = image([
        (0x1000, "root", ["sub sp, sp, #32", "bl 0x2000 <middle>", "add sp, sp, #32", "ret"]),
        (0x2000, "middle", ["sub sp, sp, #16", "add sp, sp, #16", "b 0x3000 <leaf>"]),
        (0x3000, "leaf", ["sub sp, sp, #64", "add sp, sp, #64", "ret"]),
    ])
    # 32 live caller bytes + 64 leaf bytes + one conservative 128-byte red zone.
    result = MODULE.call_region(functions, 0x1000)
    assert result["complete_stack_bound_bytes"] == 224
    assert result["complete_ordinary_call_graph"]


def test_zero_stack_tail_cycle_is_finite_positive_recursive_call_is_not():
    tail = image([(0x1000, "root", ["b 0x2000 <second>"]),
                  (0x2000, "second", ["b 0x1000 <root>"])])
    assert MODULE.call_region(tail, 0x1000)["complete_stack_bound_bytes"] == 128
    recursive = image([(0x1000, "root", ["sub sp, sp, #16", "bl 0x1000 <root>", "add sp, sp, #16", "ret"])])
    result = MODULE.call_region(recursive, 0x1000)
    assert result["complete_stack_bound_bytes"] is None
    assert result["positive_weight_recursive_components"]


def test_native_indirect_and_traps_remain_explicit():
    functions = image([(0x1000, "root", ["sub sp, sp, #32", "bl 0x9000 <stub>", "blr x8", "brk #1"])],
                      {0x9000: "_memcpy"})
    result = MODULE.call_region(functions, 0x1000)
    assert result["resolved_region_stack_bytes"] == 160
    assert result["complete_stack_bound_bytes"] is None
    assert result["frontier_kind_counts"] == {"exceptional_terminal": 1, "indirect_call": 1, "native_edge": 1}
    assert result["frontiers"][0]["detail"] == "_memcpy"
    assert all(f["caller_depth_bytes"] == 32 for f in result["frontiers"])
    indirect = function(["br x8"])
    assert indirect["gaps"][0]["kind"] == "indirect_branch"


def test_call_at_function_end_does_not_invent_return_into_next_function():
    functions = image([(0x1000, "root", ["bl 0x9000 <unknown_native>"]),
                       (0x1004, "next", ["b 0x1000 <root>"])], {0x9000: "_exit"})
    result = MODULE.call_region(functions, 0x1000)
    assert result["direct_reachable_functions"] == 1
    assert result["frontier_kind_counts"] == {"function_fallthrough": 1, "native_edge": 1}
    assert result["complete_stack_bound_bytes"] is None
    assert not result["positive_weight_recursive_components"]


def test_parsers_do_not_consume_next_line_after_operandless_return():
    text = """fixture: file format mach-o arm64
Disassembly of section __TEXT,__text:
0000000000001000 <root>:
1000: ret
0000000000001004 <next>:
1004: sub sp, sp, #0x10
1008: add sp, sp, #0x10 ; comment
100c: ret
Disassembly of section __TEXT,__stubs:
9000: br x16
"""
    instructions, names = MODULE.parse_disassembly(text)
    assert instructions == {0x1000: ("ret", ""), 0x1004: ("sub", "sp, sp, #0x10"),
                            0x1008: ("add", "sp, sp, #0x10"), 0x100c: ("ret", "")}
    assert names == {0x1000: "root", 0x1004: "next"}
    starts = "0000000000001000 _root\n0000000000001004 _next\n"
    stubs = "Indirect symbols for (__TEXT,__stubs) 1 entries\n0x0000000000009000 12 _read\nIndirect symbols for (__DATA,__got) 1 entries\n0x000000000000a000 13 _other\n"
    assert MODULE.parse_starts(starts) == {0x1000: "_root", 0x1004: "_next"}
    assert MODULE.parse_stubs(stubs) == {0x9000: "_read"}
    result = MODULE.audit(text, starts, stubs, ["root", "next"])
    assert [r["complete_stack_bound_bytes"] for r in result["roots"]] == [128, 144]
    with pytest.raises(ValueError, match="missing requested roots"):
        MODULE.audit(text, starts, stubs, ["absent"])
    with pytest.raises(ValueError, match="Mach-O ARM64"):
        MODULE.audit(text.replace("mach-o arm64", "mach-o 64-bit x86-64"), starts, stubs, ["root"])


def test_bad_disassembly_inputs_are_rejected():
    for text in ["", "Disassembly of section __TEXT,__text:\n"]:
        with pytest.raises(ValueError):
            MODULE.parse_disassembly(text)
    with pytest.raises(ValueError):
        MODULE.parse_starts("no function table")
    with pytest.raises(ValueError, match="duplicate"):
        MODULE.parse_starts("0000000000001000 a\n0000000000001000 b\n")
    with pytest.raises(ValueError, match="duplicate"):
        MODULE.parse_disassembly("Disassembly of section __TEXT,__text:\n1000: ret\n1000: ret\n")


def test_integer_parser_accepts_only_constant_immediates():
    assert MODULE.immediate("#-0x10") == -16
    assert MODULE.immediate("#0x2, lsl #12") == 8192
    for value in ["x0", "#16, lsl x0", "#1, lsl #16"]:
        with pytest.raises(ValueError):
            MODULE.immediate(value)


def test_capture_decodes_zero_blocks_and_keeps_native_and_trap_frontiers(tmp_path, monkeypatch):
    binary, tool = tmp_path / "worker", tmp_path / "llvm-objdump"
    binary.write_bytes(b"retained executable identity")
    tool.write_bytes(b"retained tool identity")
    text = """fixture: file format mach-o arm64
Disassembly of section __TEXT,__text:
0000000000001000 <root>:
1000: bl 0x9000
1004: udf #0
0000000000001008 <next>:
1008: ret
"""
    starts = "0000000000001000 root\n0000000000001008 next\n"
    stubs = "Indirect symbols for (__TEXT,__stubs) 1 entries\n0x0000000000009000 0 __exit\n"
    commands = []

    def check_output(command, **kwargs):
        return str(tool) if command == ["xcrun", "--find", "llvm-objdump"] else "fixture LLVM"

    def run(command, **kwargs):
        commands.append(command)
        if "--disassemble" in command:
            # This fixture mirrors LLVM's default elision, not a fabricated
            # return. The capture option must expose the actual trap word.
            value = text if "--disassemble-zeroes" in command else text.replace("1004: udf #0", "\t...")
        elif "--function-starts=both" in command:
            value = starts
        elif "--indirect-symbols" in command:
            value = stubs
        else:
            value = "fixture unwind metadata"
        return MODULE.subprocess.CompletedProcess(command, 0, value, "")

    monkeypatch.setattr(MODULE.subprocess, "check_output", check_output)
    monkeypatch.setattr(MODULE.subprocess, "run", run)
    raw, identity = MODULE.capture(binary, tmp_path / "raw")
    assert "--disassemble-zeroes" in commands[0]
    assert raw["disassembly"] == text
    assert identity["binary_sha256"] == MODULE.digest(binary)
    assert identity["binary_tool_and_analyzer_unchanged_during_capture"] is True
    assert identity["commands"][0]["sha256"] == MODULE.digest(tmp_path / "raw/disassembly.txt")
    result = MODULE.audit(raw["disassembly"], raw["starts"], raw["stubs"], ["root"])["roots"][0]
    assert result["frontier_kind_counts"] == {"exceptional_terminal": 1, "native_edge": 1}
    assert result["complete_stack_bound_bytes"] is None
    omitted = MODULE.audit(text.replace("1004: udf #0", "\t..."), starts, stubs, ["root"])["roots"][0]
    assert omitted["frontier_kind_counts"] == {"missing_instruction": 1, "native_edge": 1}
    assert omitted["complete_stack_bound_bytes"] is None


def terminal_fixture(callee, *, name="neutral_terminal", missing=(), identity=None):
    """Use real conditional/call/trap encodings with a separate following function."""
    root = ["cbz w0, 0x1010", "stp x29, x30, [sp, #-16]!", "mov x29, sp",
            "bl 0x1014", "ret"]
    next_start = 0x1014 + 4 * len(callee)
    lines = ["fixture: file format mach-o arm64", "Disassembly of section __TEXT,__text:"]
    starts = []
    for address, label, body in [(0x1000, "root", root), (0x1014, name, callee),
                                 (next_start, "following", ["ret"])]:
        starts.append(f"{address:016x} {label}")
        lines.append(f"{address:016x} <{label}>:")
        lines.extend(f"{address + 4 * index:x}: {instruction}" for index, instruction in enumerate(body)
                     if address + 4 * index not in missing)
    text, starts_text = "\n".join(lines) + "\n", "\n".join(starts) + "\n"
    stubs = "Indirect symbols for (__TEXT,__stubs) 1 entries\n0x0000000000003000 0 opaque_endpoint\n"
    return MODULE.audit(text, starts_text, stubs, ["root"], image_identity=identity), (text, starts_text, stubs)


@pytest.mark.parametrize("terminal", ["udf #0", "udf #65535", "brk #0x0", "brk #0xffff"])
def test_terminal_fact_omits_only_caller_continuation_and_preserves_obligations(terminal):
    result, _ = terminal_fixture(["bl 0x3000", terminal, "udf #0"])
    root = result["functions"][0]
    fact = result["terminal_callee_facts"][0]
    assert root["calls"] == [{"address": "0x100c", "target": "0x1014", "kind": "call", "caller_depth_bytes": 16}]
    assert root["maximum_sp_decrement_bytes"] == 16 and not root["frame_errors"]
    assert root["ordinary_continuations_omitted"] == [{
        "address": "0x100c", "target": "0x1014", "caller_depth_bytes": 16,
        "ordinary_successor": "0x1010", "fact_sha256": fact["fact_sha256"],
    }]
    assert fact["address"] == "0x1014" and fact["end"] == "0x1020"
    assert fact["instruction_count"] == 3 and fact["image_binding_sha256"] == result["image_binding_sha256"]
    region = result["roots"][0]
    assert region["direct_reachable_functions"] == 2
    assert region["frontier_kind_counts"] == {"exceptional_terminal": 1, "inconsistent_sp_merge": 1, "native_edge": 1}
    assert region["complete_stack_bound_bytes"] is None
    assert "no hidden exception/signal/unwind edge" in region["abi_assumptions"]
    assert not result["profile_qualified"] and not result["complete_workspace_certificate"]


@pytest.mark.parametrize("callee", [
    ["ret"], ["udf #0", "ret"], ["udf #0", "nop"],
    ["b.eq 0x1018", "udf #0"], ["b 0x1018", "udf #0"],
    ["br x0", "udf #0"], ["blr x0", "udf #0"], ["hlt #0"],
    ["bl 0xdead", "udf #0"], ["bl x0", "udf #0"],
    ["bl 0x3000 trailing", "udf #0"], ["bl 0x3000"],
    ["future x0, x1", "udf #0"], ["nop.future", "udf #0"],
    ["bit.16b v0, v1, v2", "udf #0"],
    ["sub sp, sp, x0", "udf #0"], ["mov sp, x29", "udf #0"],
    ["sub sp, sp, #8", "udf #0"], ["add sp, sp, #16", "udf #0"],
    ["ldursb w0, [sp, #-129]", "udf #0"],
    ["udf #-1"], ["brk #-1"], ["udf #65536"], ["brk #0x10000"],
    ["udf x0"], ["brk #0, lsl #12"], ["udf.future #0"],
])
def test_terminal_fact_refuses_every_unproved_control_stack_or_terminal_form(callee):
    result, _ = terminal_fixture(callee, name="panic_name_is_not_a_proof")
    assert not result["terminal_callee_facts"]
    assert not result["functions"][0]["ordinary_continuations_omitted"]
    assert result["functions"][0]["frame_errors"][0]["kind"] == "inconsistent_sp_merge"
    assert result["roots"][0]["complete_stack_bound_bytes"] is None


def test_terminal_fact_requires_complete_interval_and_known_following_boundary():
    result, _ = terminal_fixture(["nop", "udf #0"], missing={0x1014})
    assert not result["terminal_callee_facts"]
    instructions = {0x1000: ("udf", "#0")}
    assert MODULE.terminal_callee_fact(0x1000, 0x1004, instructions, {0x1000: "last"}, {}, "a" * 64) is None
    starts = {0x1000: "first", 0x1004: "next"}
    assert MODULE.terminal_callee_fact(0x1000, 0x1008, instructions, starts, {}, "a" * 64) is None
    assert MODULE.terminal_callee_fact(0x1000, 0x1004, instructions, starts, {}, None) is None
    instructions[0x1002] = ("udf", "#0")
    assert MODULE.terminal_callee_fact(0x1000, 0x1004, instructions, starts, {}, "a" * 64) is None


def test_terminal_fact_does_not_infer_transitive_nonreturn_or_erase_recursion():
    result, _ = terminal_fixture(["stp x29, x30, [sp, #-16]!", "bl 0x1014", "udf #0"])
    root = result["roots"][0]
    assert len(root["positive_weight_recursive_components"]) == 1
    assert root["frontier_kind_counts"] == {"exceptional_terminal": 1, "inconsistent_sp_merge": 1}
    assert root["complete_stack_bound_bytes"] is None
    # The terminal callee retains its full conservative analysis, including
    # the trap after the recursive call; only its external caller is refined.
    assert result["terminal_callee_facts"][0]["direct_calls"][0]["caller_depth_bytes"] == 16
    assert not result["functions"][1]["ordinary_continuations_omitted"]


def test_terminal_facts_do_not_propagate_through_a_returning_wrapper():
    text = """fixture: file format mach-o arm64
Disassembly of section __TEXT,__text:
0000000000001000 <root>:
1000: cbz w0, 0x1010
1004: stp x29, x30, [sp, #-16]!
1008: mov x29, sp
100c: bl 0x1014
1010: ret
0000000000001014 <wrapper>:
1014: bl 0x101c
1018: ret
000000000000101c <terminal>:
101c: udf #0
0000000000001020 <following>:
1020: ret
"""
    starts = "0000000000001000 root\n0000000000001014 wrapper\n000000000000101c terminal\n0000000000001020 following\n"
    result = MODULE.audit(text, starts, "", ["root"])
    assert [fact["address"] for fact in result["terminal_callee_facts"]] == ["0x101c"]
    assert not result["functions"][0]["ordinary_continuations_omitted"]
    assert result["functions"][0]["frame_errors"][0]["kind"] == "inconsistent_sp_merge"
    assert result["roots"][0]["complete_stack_bound_bytes"] is None


def test_terminal_facts_bind_full_image_metadata_binary_tool_and_interval():
    baseline, raw = terminal_fixture(["bl 0x3000", "udf #0"])
    text, starts, stubs = raw
    identity = {
        "binary_sha256": "a" * 64, "objdump_sha256": "b" * 64,
        "script_sha256": MODULE.digest(SCRIPT),
        "binary_tool_and_analyzer_unchanged_during_capture": True,
        "commands": [{"sha256": MODULE.hashlib.sha256(value.encode()).hexdigest()} for value in raw],
    }
    bound = MODULE.audit(*raw, ["root"], image_identity=identity)
    assert bound["image_binding"]["binary_sha256"] == identity["binary_sha256"]
    assert bound["image_binding"]["objdump_sha256"] == identity["objdump_sha256"]
    assert bound["terminal_callee_facts"][0]["fact_sha256"] != baseline["terminal_callee_facts"][0]["fact_sha256"]
    changed = MODULE.audit(text + "\n", starts, stubs, ["root"])
    assert changed["image_binding_sha256"] != baseline["image_binding_sha256"]
    assert changed["terminal_callee_facts"][0]["interval_sha256"] == baseline["terminal_callee_facts"][0]["interval_sha256"]
    assert changed["terminal_callee_facts"][0]["fact_sha256"] != baseline["terminal_callee_facts"][0]["fact_sha256"]
    with pytest.raises(ValueError, match="does not bind"):
        MODULE.audit(text + "\n", starts, stubs, ["root"], image_identity=identity)
    for field in ("binary_sha256", "objdump_sha256", "script_sha256"):
        with pytest.raises(ValueError, match="identity"):
            MODULE.audit(*raw, ["root"], image_identity={**identity, field: "not-a-digest"})
    changed_binary = MODULE.audit(*raw, ["root"], image_identity={**identity, "binary_sha256": "c" * 64})
    assert changed_binary["image_binding_sha256"] != bound["image_binding_sha256"]


def test_caller_rejects_stale_image_or_modified_interval_fact():
    result, raw = terminal_fixture(["bl 0x3000", "udf #0"])
    instructions, names = MODULE.parse_disassembly(raw[0])
    starts, stubs = MODULE.parse_starts(raw[1]), MODULE.parse_stubs(raw[2])
    fact = result["terminal_callee_facts"][0]
    for binding, current in [("different-image", instructions),
                             (result["image_binding_sha256"], {**instructions, 0x1018: ("ret", "")})]:
        fn = MODULE.analyze_function(0x1000, 0x1014, current, starts, stubs, names[0x1000],
                                     {0x1014: fact}, binding)
        assert any(error["kind"] == "invalid_terminal_callee_fact" for error in fn["frame_errors"])
        assert not fn["ordinary_continuations_omitted"]
        assert fn["calls"][0]["caller_depth_bytes"] == 16


def test_omitted_continuation_retains_native_unwind_and_trap_obligations():
    text = """fixture: file format mach-o arm64
Disassembly of section __TEXT,__text:
0000000000001000 <root>:
1000: cbz w0, 0x1014
1004: stp x29, x30, [sp, #-16]!
1008: bl 0x1018
100c: bl 0x3000
1010: udf #0
1014: ret
0000000000001018 <terminal>:
1018: udf #0
000000000000101c <following>:
101c: ret
"""
    starts = "0000000000001000 root\n0000000000001018 terminal\n000000000000101c following\n"
    stubs = "Indirect symbols for (__TEXT,__stubs) 1 entries\n0x0000000000003000 0 __Unwind_Resume\n"
    result = MODULE.audit(text, starts, stubs, ["root"])
    fn = result["functions"][0]
    assert not fn["gaps"] and not fn["frame_errors"]
    deferred = fn["deferred_continuation_frontiers"]
    assert [(row["address"], row["kind"], row["caller_depth_bytes"]) for row in deferred] == [
        ("0x100c", "native_edge", 16), ("0x1010", "exceptional_terminal", 16),
    ]
    assert deferred[0]["detail"] == "__Unwind_Resume"
    fact = result["terminal_callee_facts"][0]
    assert all(row["refinement_fact_sha256s"] == [fact["fact_sha256"]] for row in deferred)
    assert all(row["omitted_call_addresses"] == ["0x1008"] for row in deferred)
    assert fn["conservative_analysis_before_refinement"]["gaps"][0]["detail"] == "__Unwind_Resume"
    assert result["roots"][0]["frontier_kind_counts"] == {"exceptional_terminal": 2, "native_edge": 1}
    assert result["roots"][0]["complete_stack_bound_bytes"] is None


def test_deferred_frontier_itself_refuses_a_complete_bound():
    fn = function(["ret"])
    fn["deferred_continuation_frontiers"] = [{
        "address": "0x1004", "kind": "native_edge", "detail": "opaque_native",
        "caller_depth_bytes": 0, "refinement_fact_sha256s": ["a" * 64],
        "omitted_call_addresses": ["0x1000"],
    }]
    result = MODULE.call_region({0x1000: fn}, 0x1000)
    assert result["frontier_kind_counts"] == {"native_edge": 1}
    assert not result["complete_ordinary_call_graph"] and result["complete_stack_bound_bytes"] is None


def test_deferred_call_preserves_downstream_positive_weight_recursion():
    text = """fixture: file format mach-o arm64
Disassembly of section __TEXT,__text:
0000000000001000 <root>:
1000: cbz w0, 0x1010
1004: bl 0x1014
1008: bl 0x1018
100c: ret
1010: ret
0000000000001014 <terminal>:
1014: udf #0
0000000000001018 <recursive>:
1018: stp x29, x30, [sp, #-16]!
101c: bl 0x1018
1020: add sp, sp, #16
1024: ret
0000000000001028 <following>:
1028: ret
"""
    starts = "0000000000001000 root\n0000000000001014 terminal\n0000000000001018 recursive\n0000000000001028 following\n"
    result = MODULE.audit(text, starts, "", ["root"])
    fn = result["functions"][0]
    assert [row["target"] for row in fn["calls"]] == ["0x1014"]
    assert [row["target"] for row in fn["deferred_continuation_calls"]] == ["0x1018"]
    assert fn["deferred_continuation_calls"][0]["caller_depth_bytes"] == 0
    assert fn["deferred_continuation_calls"][0]["omitted_call_addresses"] == ["0x1004"]
    region = result["roots"][0]
    assert region["direct_reachable_functions"] == 3
    assert region["positive_weight_recursive_components"] == [[{"address": "0x1018", "name": "recursive"}]]
    assert region["complete_stack_bound_bytes"] is None


def test_boundary_fallthrough_after_omitted_call_remains_a_deferred_obligation():
    text = """fixture: file format mach-o arm64
Disassembly of section __TEXT,__text:
0000000000001000 <root>:
1000: cbz w0, 0x1008
1004: ret
1008: bl 0x100c
000000000000100c <terminal>:
100c: brk #0
0000000000001010 <following>:
1010: ret
"""
    starts = "0000000000001000 root\n000000000000100c terminal\n0000000000001010 following\n"
    result = MODULE.audit(text, starts, "", ["root"])
    assert result["functions"][0]["deferred_continuation_frontiers"][0]["kind"] == "function_fallthrough"
    assert result["roots"][0]["frontier_kind_counts"] == {"exceptional_terminal": 1, "function_fallthrough": 1}
    assert result["roots"][0]["complete_stack_bound_bytes"] is None


def test_stack_growing_loop_after_omitted_call_stays_an_active_merge_refusal():
    text = """fixture: file format mach-o arm64
Disassembly of section __TEXT,__text:
0000000000001000 <root>:
1000: bl 0x100c
1004: sub sp, sp, #16
1008: b 0x1004
000000000000100c <terminal>:
100c: udf #0
0000000000001010 <following>:
1010: ret
"""
    starts = "0000000000001000 root\n000000000000100c terminal\n0000000000001010 following\n"
    result = MODULE.audit(text, starts, "", ["root"])
    fn = result["functions"][0]
    assert not fn["frame_errors"] and fn["ordinary_maximum_sp_decrement_bytes"] == 0
    assert fn["maximum_sp_decrement_bytes"] == 16
    deferred = fn["deferred_continuation_frontiers"]
    assert len(deferred) == 1 and deferred[0]["kind"] == "inconsistent_sp_merge"
    assert deferred[0]["address"] == "0x1004" and deferred[0]["caller_depth_bytes"] == 16
    assert deferred[0]["omitted_call_addresses"] == ["0x1000"]
    assert fn["ordinary_sp_merge_explanations"][0]["status"] == "retained as an active deferred obligation"
    assert result["whole_image_ordinary_frame_error_count"] == 0
    assert result["whole_image_frame_error_count"] == 1
    assert result["roots"][0]["frontier_kind_counts"] == {"exceptional_terminal": 1, "inconsistent_sp_merge": 1}
    assert result["roots"][0]["complete_stack_bound_bytes"] is None
