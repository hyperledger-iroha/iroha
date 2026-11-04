#!/usr/bin/env python3
"""Compose the retained Reservation wrapper with its exact decode_record leaf.

Python 3.11+ and the original November 4 cache/source/tool bytes are required.
No environment variables, subprocesses, builds or native runs. --output must be
new and external. Explicit reviewed helper/test SHA arguments are mandatory.
Only wrapper and record-leaf instruction events are counted. Every other callee
and exceptional entry remains opaque; this supplies no complete resource bound.
"""
import argparse
from collections import deque
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
from types import ModuleType


START, END = 0x100000B40, 0x100000E84
LEAF_HELPER = 'scripts/parliament_ballot_retained_record_bound.py'
LEAF_TESTS = 'scripts/tests/parliament_ballot_retained_record_bound_test.py'
LEAF_HELPER_SHA = '8f9502d04178765e7e48c0d5dbe3046fe0a9b2c5b4c97f04e1ce5a6fb6961c81'
LEAF_TESTS_SHA = '2fc2c7317c87479acf71b5ca1e1525d26021364e2a7d471f42ba0acdd5e895fd'
LEAF_REPORT = 'record-leaf-20261104T145500Z/report.json'
LEAF_REPORT_SHA = '95b73bececa3ce01585704f95e774c8e41f5c3672477741576810e50ec8d8da3'
LEAF_REVIEW = 'record-leaf-independent-review-20261104T145449Z/capture-review.json'
LEAF_REVIEW_SHA = '21055c453544d4e3aabd04e0f533a99430a83bc50923647327570853b60bc160'
OPAQUE_NAMES = {
    '<iroha_parliament_ballot_proof::profile::Profile>::new': 'profile',
    'iroha_parliament_ballot_proof::test_codec::decode_vote': 'typed_vote',
    'iroha_parliament_ballot_proof::test_codec::decode_registration': 'typed_registration',
    'iroha_parliament_ballot_proof::vote::verify': 'role_vote',
    'iroha_parliament_ballot_proof::registration::verify': 'role_registration',
    '__rustc::__rust_dealloc': 'dealloc',
    'core::ptr::drop_glue::<iroha_parliament_ballot_proof::credential::RegistrationContext>': 'context_drop',
    '_memcpy': 'memcpy',
}


def require(condition, message):
    """Refuse an unsupported condition, without a partial success verdict."""
    if not condition:
        raise ValueError(message)


def sha(data):
    """Hash the exact bytes retained or executed as the verified leaf module."""
    return hashlib.sha256(data).hexdigest()


def pinned(path, expected, guards):
    """Original/reviewed pins are authority; a changed input is never repinned."""
    require(re.fullmatch('[0-9a-f]{64}', expected) is not None, 'invalid SHA-256')
    data = Path(path).read_bytes()
    require(sha(data) == expected, 'pinned input changed: ' + str(path))
    require(str(path) not in guards or guards[str(path)] == expected, 'conflicting input pin')
    guards[str(path)] = expected
    return data


def load_leaf(root, guards):
    """Execute only the exact verified source buffer, avoiding import-cache drift."""
    path = root / LEAF_HELPER
    data = pinned(path, LEAF_HELPER_SHA, guards)
    pinned(root / LEAF_TESTS, LEAF_TESTS_SHA, guards)
    module = ModuleType('verified_retained_record_leaf')
    module.__file__ = str(path)
    exec(compile(data, str(path), 'exec'), module.__dict__)
    return module


def instruction(op, operands):
    """Validate the finite wrapper's exact ordinary forms and explicit SP effects.

    All integers/registers use ASCII LLVM spelling. This is deliberately not a
    general ISA decoder. Memory validity, LR restoration from valid storage and
    callee effects are external ordinary-ABI premises, not memory-safety proofs.
    """
    operands = re.sub(r' <.*>$', '', operands)
    g = r'(?:[0-9]|1[0-9]|2[0-8])'
    x, w, base = 'x' + g, 'w' + g, rf'(?:x{g}|sp)'
    immediate = r'#(-?(?:0x[0-9a-f]+|[0-9]+))'
    writes, delta, target, kind = [], 0, None, 'straight'
    if op == 'ret':
        require(operands == '', 'unsupported RET operands')
        kind = 'return'
    elif op in {'b', 'bl', 'b.eq', 'b.ne'}:
        require(re.fullmatch(r'0x[0-9a-f]+', operands) is not None, 'unsupported direct target')
        target = int(operands, 16)
        kind = 'call' if op == 'bl' else ('jump' if op == 'b' else 'conditional')
    elif op in {'cbz', 'cbnz'}:
        match = re.fullmatch(rf'[wx]{g}, (0x[0-9a-f]+)', operands)
        require(match is not None, 'unsupported compare branch')
        target, kind = int(match[1], 16), 'conditional'
    elif op in {'add', 'sub'}:
        match = re.fullmatch(rf'({x}|sp|x29), ({base}), {immediate}', operands)
        require(match is not None and 0 <= int(match[3], 0) <= 4095, 'unsupported ADD/SUB operands')
        destination, source, amount = match[1], match[2], int(match[3], 0)
        require(destination != 'x29' or (op == 'add' and source == 'sp'), 'unsupported FP assignment')
        if destination == 'sp':
            require(source == 'sp', 'unknown SP assignment')
            delta = amount if op == 'add' else -amount
        writes = [destination]
    elif op in {'stp', 'ldp'}:
        # FP/LR appear only as the saved/restored pair. No integer SP register
        # is accepted as data, and all writeback forms have an explicit base.
        pair = rf'(?:{x}|x29|x30)'
        match = re.fullmatch(rf'({pair}), ({pair}), \[({base})(?:, {immediate})?\](!)?(?:, {immediate})?', operands)
        require(match is not None, 'unsupported paired memory operands')
        first, second, address_base = match[1], match[2], match[3]
        offset, pre, post = int(match[4] or '0', 0), bool(match[5]), match[6]
        require(not (pre and post) and (post is None or match[4] is None), 'ambiguous paired writeback')
        amount = int(post, 0) if post is not None else offset
        require(-512 <= amount <= 504 and amount % 8 == 0, 'paired offset out of range')
        if first in {'x29', 'x30'} or second in {'x29', 'x30'}:
            require((first, second, address_base, offset, pre, post) == ('x29', 'x30', 'sp', 80, False, None),
                    'unsupported saved FP/LR pair')
        require(op != 'ldp' or first != second, 'overlapping paired destinations')
        if pre or post is not None:
            require(address_base == 'sp', 'unsupported non-SP paired writeback')
            delta = amount
            writes.append('sp')
        if op == 'ldp':
            writes.extend([first, second])
    elif op in {'ldr', 'ldrb', 'ldur', 'ldurh', 'str'}:
        register = rf'(?:{x}|xzr)' if op in {'ldr', 'str'} else w
        match = re.fullmatch(rf'({register}), \[({base})(?:, {immediate})?\]', operands)
        require(match is not None, 'unsupported scalar memory operands')
        offset = int(match[3] or '0', 0)
        if op in {'ldur', 'ldurh'}:
            require(-256 <= offset <= 255, 'unscaled offset out of range')
        else:
            scale = 8 if op in {'ldr', 'str'} else 1
            require(0 <= offset <= 4095 * scale and offset % scale == 0, 'scaled offset out of range')
        if op != 'str':
            writes = [match[1]]
    elif op == 'mov':
        match = re.fullmatch(rf'([wx])({g}), (?:\1{g}|{immediate})', operands)
        require(match is not None and (match[3] is None or -1 <= int(match[3], 0) <= 65535), 'unsupported MOV operands')
        writes = [match[1] + match[2]]
    elif op in {'cmp', 'cmn'}:
        match = re.fullmatch(rf'([wx]){g}, (?:\1{g}|{immediate})', operands)
        require(match is not None and (match[2] is None or 0 <= int(match[2], 0) <= 4095), 'unsupported compare operands')
    elif op in {'lsl', 'lsr'}:
        match = re.fullmatch(rf'({x}), {x}, {immediate}', operands)
        require(match is not None and 0 <= int(match[2], 0) < 64, 'unsupported shift operands')
        writes = [match[1]]
    elif op == 'orr':
        match = re.fullmatch(rf'([wx])({g}), \1{g}, (?:#0x1|\1{g}, lsl {immediate})', operands)
        require(match is not None and (match[3] is None or 0 <= int(match[3], 0) < (64 if match[1] == 'x' else 32)),
                'unsupported ORR operands')
        writes = [match[1] + match[2]]
    elif op == 'bfi':
        match = re.fullmatch(rf'({x}), {x}, {immediate}, {immediate}', operands)
        require(match is not None and 0 <= int(match[2], 0) < 64
                and 1 <= int(match[3], 0) <= 64 - int(match[2], 0), 'unsupported BFI operands')
        writes = [match[1]]
    else:
        raise ValueError('unsupported wrapper opcode: ' + op)
    return {'opcode': op, 'operands': operands, 'kind': kind, 'target': target, 'sp_delta': delta,
            'writes': [('x' + value[1:]) if value.startswith('w') else value for value in writes]}


def topology(edges, entry):
    """Require every reachable edge internal and prove the ordinary graph a DAG."""
    require(entry in edges and all(target in edges for values in edges.values() for target in values), 'ordinary interval escape')
    degree = dict.fromkeys(edges, 0)
    for targets in edges.values():
        for target in targets:
            degree[target] += 1
    pending, order = deque(sorted(a for a in edges if degree[a] == 0)), []
    while pending:
        address = pending.popleft()
        order.append(address)
        for target in edges[address]:
            degree[target] -= 1
            if degree[target] == 0:
                pending.append(target)
    require(len(order) == len(edges), 'ordinary CFG cycle')
    return order


def longest(edges, order, weights, entry):
    """Bound an additive instruction/count/request metric, not opaque callee cost."""
    lengths, following = {}, {}
    for address in reversed(order):
        weight = weights.get(address, 0)
        require(type(weight) is int and weight >= 0, 'invalid path weight')
        if edges[address]:
            target = max(edges[address], key=lambda a: (lengths[a], -a))
            lengths[address], following[address] = weight + lengths[target], target
        else:
            lengths[address] = weight
    path, address = [], entry
    while True:
        path.append(hex(address))
        if address not in following:
            break
        address = following[address]
    return {'maximum': lengths[entry], 'cfg_path': path, 'data_feasibility_not_claimed': True}


def frame_states(words, edges, order, entry):
    """Derive SP from explicit writes, ordinary call preservation and every merge."""
    before, after, maximum = {entry: 0}, {}, 0
    for address in order:
        require(address in before, 'unreached SP state')
        value = before[address] + words[address]['sp_delta']
        require(value <= 0 and value % 16 == 0, 'invalid SP state')
        after[address] = value
        maximum = max(maximum, -before[address], -value)
        if words[address]['kind'] == 'return':
            require(value == 0, 'unbalanced ordinary return')
        for target in edges[address]:
            require(target not in before or before[target] == value, 'inconsistent SP merge')
            before[target] = value
    return before, after, maximum


def request_length(address, words, edges):
    """Find an unambiguous constant x2 definition before one native memcpy BL."""
    predecessors = {a: [] for a in edges}
    for source, targets in edges.items():
        for target in targets:
            predecessors[target].append(source)
    while True:
        require(len(predecessors[address]) == 1, 'ambiguous memcpy request definition')
        address = predecessors[address][0]
        word = words[address]
        require(word['kind'] == 'straight', 'memcpy request crosses a call or control boundary')
        if 'x2' in word['writes']:
            match = re.fullmatch(r'w2, #(0x[0-9a-f]+|[0-9]+)', word['operands'])
            require(word['opcode'] == 'mov' and match is not None, 'nonconstant memcpy request')
            return int(match[1], 0)


def compose(leaf, raw, original, reviewed_leaf, start=START, end=END):
    """Derive one ordinary wrapper/leaf projection and retain the entire interval."""
    instructions, starts = leaf.parse_text(raw['disassembly']), leaf.parse_starts(raw['starts'])
    require(start in starts and starts.index(start) + 1 < len(starts)
            and starts[starts.index(start) + 1] == end, 'exact successor interval missing')
    addresses = list(range(start, end, 4))
    require([a for a in sorted(instructions) if start <= a < end] == addresses, 'incomplete wrapper interval')
    words = {a: instruction(*instructions[a]) for a in addresses}
    by_address = {int(row['address'], 16): row for row in original['functions']}
    require(start in by_address and by_address[start]['end'] == hex(end), 'v5 wrapper interval differs')
    stubs, active = {}, False
    for line in raw['stubs'].splitlines():
        if line.startswith('Indirect symbols for '):
            active = '(__TEXT,__stubs)' in line
        match = re.fullmatch(r'(0x[0-9a-fA-F]+)\s+[0-9]+\s+(\S+)', line)
        if active and match:
            require(int(match[1], 16) not in stubs, 'duplicate native stub')
            stubs[int(match[1], 16)] = match[2]
    derived_leaf = leaf.leaf_bound(raw['disassembly'], raw['starts'])
    require(derived_leaf == reviewed_leaf['leaf'] and reviewed_leaf['original_image_binding'] == original['image_binding']
            and reviewed_leaf['inherited_outer_roots_unchanged'] == original['roots'], 'reviewed leaf image/bound/roots differ')
    all_edges, calls = {}, {}
    for address, word in words.items():
        kind, target = word['kind'], word['target']
        if kind == 'call':
            require(target in by_address or target in stubs, 'unknown exact callee entry')
            name = stubs.get(target) or by_address[target]['name']
            label = 'record' if target == leaf.START else OPAQUE_NAMES.get(name, 'opaque:' + name)
            calls[address] = {'address': hex(address), 'target': hex(target), 'symbol': name, 'label': label}
        all_edges[address] = ([] if kind == 'return' else [target] if kind == 'jump'
                              else [target, address + 4] if kind == 'conditional' else [address + 4])
    pending, edges = [start], {}
    while pending:
        address = pending.pop()
        if address in edges:
            continue
        require(address in words, 'ordinary interval escape')
        edges[address] = all_edges[address]
        pending.extend(edges[address])
    order = topology(edges, start)
    before, after, maximum = frame_states(words, edges, order, start)
    ordinary_calls = {a: row for a, row in calls.items() if a in edges}
    for address, row in ordinary_calls.items():
        row.update(caller_depth_bytes=-before[address], composed_record_leaf=row['label'] == 'record',
                   complete_callee_work_bound=None, complete_callee_stack_bound_bytes=None)
    local = longest(edges, order, dict.fromkeys(edges, 1), start)
    weighted = longest(edges, order, {a: 1 + (derived_leaf['ordinary_instruction_events_upper_bound']
                                             if a in ordinary_calls and ordinary_calls[a]['label'] == 'record' else 0)
                                     for a in edges}, start)
    labels = sorted({row['label'] for row in ordinary_calls.values()})
    coefficients = {label: longest(edges, order, {a: int(row['label'] == label) for a, row in ordinary_calls.items()}, start)
                    for label in labels}
    for group, values in {'all_direct_calls': labels, 'typed_decoder_calls': ['typed_vote', 'typed_registration'],
                          'role_verifier_calls': ['role_vote', 'role_registration'], 'direct_cleanup_calls': ['dealloc', 'context_drop']}.items():
        coefficients[group] = longest(edges, order, {a: int(row['label'] in values) for a, row in ordinary_calls.items()}, start)
    requests = {a: request_length(a, words, edges) for a, row in ordinary_calls.items() if row['label'] == 'memcpy'}
    old = by_address[start]
    require(old['frame_errors'] == [] and old['ordinary_continuations_omitted'] == []
            and old['visited_instructions'] == len(edges) and old['maximum_sp_decrement_bytes'] == maximum
            and old['local_frame_plus_red_zone_bytes'] == maximum + original['red_zone_bytes'], 'v5 frame observation differs')
    expected_calls = sorted((int(row['address'], 16), int(row['target'], 16), row['caller_depth_bytes']) for row in old['calls'])
    actual_calls = sorted((a, int(row['target'], 16), row['caller_depth_bytes']) for a, row in ordinary_calls.items()
                          if int(row['target'], 16) not in stubs)
    expected_native = sorted((int(row['address'], 16), row['detail'], row['caller_depth_bytes']) for row in old['gaps'])
    actual_native = sorted((a, row['symbol'], row['caller_depth_bytes']) for a, row in ordinary_calls.items()
                           if int(row['target'], 16) in stubs)
    require(expected_calls == actual_calls and expected_native == actual_native, 'v5 call/frontier inventory differs')
    inventory = [{'address': hex(a), **words[a], 'successors': [hex(b) for b in all_edges[a]],
                  'ordinary_entry_reachable': a in edges, 'entry_relative_sp_before': before.get(a),
                  'entry_relative_sp_after': after.get(a)} for a in addresses]
    outside = [row for row in inventory if not row['ordinary_entry_reachable']]
    outside_calls = [{**row, 'caller_depth_bytes': None, 'entry_condition': None,
                      'complete_callee_work_bound': None, 'complete_callee_stack_bound_bytes': None}
                     for a, row in sorted(calls.items()) if a not in edges]
    return {'interval_start': hex(start), 'interval_end_exclusive': hex(end), 'decoded_words': len(addresses),
            'ordinary_entry_reachable_words': len(edges), 'full_interval_inventory': inventory,
            'local_ordinary_instruction_events': local, 'wrapper_and_record_ordinary_instruction_events': weighted,
            'instruction_scope': 'One ordinary wrapper activation plus its direct record-leaf activations; excludes every opaque callee and every exceptional/reentrant activation.',
            'direct_call_coefficients': coefficients, 'ordinary_calls': [row for _, row in sorted(ordinary_calls.items())],
            'native_memcpy_request_bytes': {'sites': {hex(a): n for a, n in requests.items()},
                                          'path_bound': longest(edges, order, requests, start),
                                          'native_work_or_memory_footprint_bound': None},
            'maximum_explicit_local_sp_decrement_bytes': maximum,
            'inherited_v5_modeled_local_frame_plus_red_zone_bytes': old['local_frame_plus_red_zone_bytes'],
            'nonentry_obligations': {'status': 'active unresolved exceptional-entry obligations, not dead code',
                                    'words': outside, 'calls': outside_calls, 'entry_sp_bytes': None,
                                    'work_bound': None, 'stack_bound_bytes': None,
                                    'out_of_interval_successors': [{'source': hex(a), 'target': hex(b)}
                                        for a in words if a not in edges for b in all_edges[a] if b not in words]},
            'complete_work_bound': None, 'complete_physical_stack_bound_bytes': None, 'pointer_memory_bound': None,
            'elapsed_time_bound': None, 'exceptional_path_bound': None, 'caller_bound': None,
            'inherited_outer_roots_unchanged': original['roots'], 'original_image_binding': original['image_binding']}


def run(root, cache, output, helper_sha256, tests_sha256):
    """Retain a new result/refusal with original pins and unknown costs explicit."""
    root, cache, output = (Path(path).resolve() for path in (root, cache, output))
    require(not output.is_relative_to(root) and not cache.is_relative_to(output)
            and (not output.is_relative_to(cache) or (output.parent == cache and output.name.startswith('record-frontend-'))),
            'output must be separate external record-frontend evidence')
    output.mkdir(parents=True, exist_ok=False)
    report = {'schema': 'parliament-retained-frontend-composition-v1', 'observed_utc': datetime.now(timezone.utc).isoformat(),
              'analysis_passed': False, 'failure': None, 'modes': {}, 'input_sha256_before': {}, 'input_sha256_after': {},
              'scope': 'Ordinary wrapper/record instruction projection only, with all other callees and exceptional entries opaque.',
              'latest_source_build_validated': False, 'historical_evidence_rebound': False, 'complete_stack_certificate': False,
              'complete_workspace_certificate': False, 'profile_qualified': False, 'release_qualified': False,
              'physical_cost_calibrated': False, 'no_build_disassembler_or_worker_execution': True,
              'premises': ['Original retained image/tool/metadata decoding and valid immutable compiled code.',
                           'One valid Rust/ARM64 ABI entry; memory accesses and restored return address obey that ABI.',
                           'Opaque calls may return ordinarily with caller SP and callee-saved registers preserved; no return or cost is guaranteed.',
                           'Caller-pointer memory, faults, traps, hidden unwind, signals and reentry remain unbounded.',
                           'The inherited 128-byte red-zone model is not a complete physical stack certificate.']}
    guards = report['input_sha256_before']
    try:
        helper = pinned(Path(__file__).resolve(), helper_sha256, guards)
        tests = pinned(root / 'scripts/tests/parliament_ballot_retained_frontend_bound_test.py', tests_sha256, guards)
        for name, data in [('helper.py.template', helper), ('tests.py.template', tests)]:
            with (output / name).open('xb') as stream:
                stream.write(data)
        leaf = load_leaf(root, guards)
        reviewed = leaf.object_json(pinned(cache / LEAF_REPORT, LEAF_REPORT_SHA, guards))
        pinned(cache / LEAF_REVIEW, LEAF_REVIEW_SHA, guards)
        require(reviewed['analysis_passed'] is True and reviewed['failure'] is None
                and reviewed['input_sha256_before'] == reviewed['input_sha256_after'], 'original leaf capture refused')
        modes, triage = leaf.load_inputs(root, cache, guards)
        for mode, (raw, original) in modes.items():
            report['modes'][mode] = compose(leaf, raw, original, reviewed['modes'][mode])
        report['record_leaf_report_sha256'] = LEAF_REPORT_SHA
        report['historical_selected_nodes'] = reviewed['historical_selected_nodes']
        report['prior_observed_build_input_drift'] = triage['build_inputs_different_from_retained_capture']
        report['live_source_scope'] = 'Only the original nine targeted source files; no full build/source freshness claim.'
        report['input_sha256_after'] = leaf.recheck(guards)
        report['analysis_passed'] = True
    except (OSError, ValueError, KeyError, TypeError, IndexError) as error:
        report['failure'] = type(error).__name__ + ': ' + str(error)
    with (output / 'report.json').open('x') as stream:
        stream.write(json.dumps(report, indent=2, sort_keys=True) + '\n')
    return report


def main():
    """Require explicit paths and reviewed source/test identities."""
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('root', 'cache', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--helper-sha256', required=True)
    parser.add_argument('--tests-sha256', required=True)
    args = parser.parse_args()
    result = run(args.root, args.cache, args.output, args.helper_sha256, args.tests_sha256)
    print(json.dumps({'analysis_passed': result['analysis_passed'], 'failure': result['failure'],
                      'report': str(args.output.resolve() / 'report.json')}))
    return 0 if result['analysis_passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
