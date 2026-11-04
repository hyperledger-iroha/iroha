#!/usr/bin/env python3
"""Bound one ordinary decode_record entry in the retained November 4 ARM64 images.

Python 3.11+, the original Parliament cache and matching targeted source/tool
bytes are required. No environment variables, build, subprocess or native run.
--output must be a new external directory. Reviewed helper/test hashes are
mandatory. This is an instruction-event sublemma, not a time, memory-safety,
physical-resource, verifier, production-codec or latest-source certificate.
"""
import argparse
from collections import deque
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re


START, END = 0x10000C2A4, 0x10000C380
MODES = ('portable', 'accel')
PINS = {
    'build/build-receipt.json': 'ac3e4ddc0685ec1f000fa809cccd4c060ecc9899010ed1b9d7047bde2659817a',
    'stack-v5/receipt.json': 'a2f2883e1d7576a8938ad0d87b8a39ba7e200d7ad6c8275cfc69915949ff1287',
    'stack-v5/portable.json': '0498e2ec5dc4c54d3c21f16b6975e6276809f38f60f759ef4de807327ad35f22',
    'stack-v5/accel.json': '8804a8328172dde9ebea86bd706921d1cdcf70ee16d496be235d4087fda5cd93',
    'next-campaign-review-20261104T055051Z/review.json': 'bad1f65c971d7393b4979e7117c55c98c41d005bf90a9d03ead05888382bf75d',
}
SOURCE_SUFFIXES = (
    'src/test_codec.rs', 'src/profile.rs', 'examples/resource_contract/budget.rs',
    'examples/resource_contract/arena.rs', 'examples/resource_contract/native.rs',
    'examples/resource_contract/workspace.rs', 'examples/memory_entry/driver.rs',
    'examples/memory_entry/fatal.rs',
)
SCOPE = ('Only ordinary instruction events within one exact retained function interval; '
         'both branch outcomes included, with no caller or exceptional-path composition.')


def require(condition, message):
    """Refuse an unproved condition without manufacturing a partial success."""
    if not condition:
        raise ValueError(message)


def sha(data):
    """Hash the bytes actually parsed or retained."""
    return hashlib.sha256(data).hexdigest()


def object_json(data):
    """Reject duplicate keys and nonfinite constants in custody documents."""
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, 'duplicate JSON key')
            result[key] = value
        return result

    def bad(_):
        raise ValueError('nonfinite JSON constant')

    return json.loads(data, object_pairs_hook=unique, parse_constant=bad)


def pinned(path, expected, guards):
    """Read once, check the original/reviewed pin, and bind those same bytes."""
    path = Path(path)
    require(re.fullmatch('[0-9a-f]{64}', expected) is not None, 'invalid SHA-256 pin')
    data = path.read_bytes()
    require(sha(data) == expected, 'pinned input changed: ' + str(path))
    require(str(path) not in guards or guards[str(path)] == expected, 'conflicting input pin')
    guards[str(path)] = expected
    return data


def parse_text(text):
    """Parse every printed word in __TEXT,__text; reject ambiguous/unknown lines."""
    marker = 'Disassembly of section __TEXT,__text:'
    require(text.count(marker) == 1, 'missing or repeated text section')
    body = text.split(marker, 1)[1].split('\nDisassembly of section ', 1)[0]
    instructions = {}
    for line in body.splitlines():
        if not line.strip() or re.fullmatch(r'[0-9a-fA-F]+ <.+>:', line.strip()):
            continue
        match = re.fullmatch(r'\s*([0-9a-fA-F]+):[ \t]+(\S+)(?:[ \t]+(.*))?', line)
        require(match is not None, 'unparsed instruction line')
        address = int(match[1], 16)
        require(address not in instructions and address % 4 == 0, 'duplicate or misaligned word')
        operands = (match[3] or '').split(';', 1)[0].strip()
        # LLVM symbol annotations are descriptive, never branch authority.
        operands = re.sub(r' <[^<>]+>$', '', operands)
        instructions[address] = (match[2], operands)
    require(bool(instructions), 'empty text section')
    return instructions


def parse_starts(text):
    """Require unique aligned LC_FUNCTION_STARTS records, not local labels."""
    starts = []
    lines = text.splitlines()
    require(bool(lines) and lines[0].endswith(':'), 'missing starts file header')
    for line in lines[1:]:
        if not line.strip():
            continue
        match = re.fullmatch(r'([0-9a-fA-F]{16})\s+\S+', line)
        require(match is not None, 'malformed function start')
        address = int(match[1], 16)
        require(address % 4 == 0 and address not in starts, 'duplicate or misaligned function start')
        starts.append(address)
    require(starts == sorted(starts), 'unordered function starts')
    return starts


def successors(address, op, operands):
    """Accept only reviewed ordinary forms present in this leaf, without SP/FP/LR.

    W/X0..28 are ordinary registers; no accepted memory form has SP as base.
    The two post-indexed LDRs update only an ordinary integer base. All other
    accepted non-branch instructions fall through. Loads/stores may fault: this
    function does not model or bound those exceptional paths or their handlers.
    """
    x = r'x(?:[0-9]|1[0-9]|2[0-8])'
    w = r'w(?:[0-9]|1[0-9]|2[0-8])'
    imm = r'#(?:0x[0-9a-f]+|[0-9]+)'
    if op == 'ret':
        require(operands == '', 'unsupported RET operands')
        return []
    if op in {'b.hi', 'b.hs', 'b.ls', 'b.ne'}:
        require(re.fullmatch(r'0x[0-9a-f]+', operands) is not None, 'unknown branch target')
        return [int(operands, 16), address + 4]
    patterns = {
        'cmp': rf'(?:{x}, (?:{x}|{imm})|{w}, (?:{w}|{imm}))',
        'mov': rf'{w}, {imm}',
        'movk': rf'{w}, {imm}, lsl #16',
        'add': rf'{x}, {x}, (?:{x}|{imm})',
        'sub': rf'{x}, {x}, (?:{x}|{imm})',
        'subs': rf'{x}, {x}, (?:{x}|{imm})',
        'csel': rf'{x}, {x}, {x}, ne',
        'ldr': rf'{w}, \[{x}\], #0x4',
        'ldrb': rf'{w}, \[{x}\]',
        'strb': rf'{w}, \[{x}\]',
        'str': rf'{x}, \[{x}, {imm}\]',
        'stp': rf'{x}, {x}, \[{x}, {imm}\]',
    }
    require(op in patterns and re.fullmatch(patterns[op], operands) is not None,
            'unsupported opcode or operands: ' + op + ' ' + operands)
    numbers = [int(value, 0) for value in re.findall(r'#(0x[0-9a-f]+|[0-9]+)', operands)]
    if op in {'mov', 'movk'}:
        require(numbers[0] <= 65535, 'wide-move immediate out of range')
    elif op in {'cmp', 'add', 'sub', 'subs'} and numbers:
        require(numbers[0] <= 4095, 'arithmetic immediate out of range')
    elif op in {'str', 'stp'}:
        limit = 32760 if op == 'str' else 504
        require(numbers[0] <= limit and numbers[0] % 8 == 0, 'store offset out of range')
    if op == 'ldr':
        destination, base = re.findall(r'\b[wx][0-9]+\b', operands)
        require(destination[1:] != base[1:], 'constrained-unpredictable LDR writeback overlap')
    return [address + 4]


def leaf_bound(disassembly, function_starts, start=START, end=END):
    """Derive complete leaf coverage, a DAG, and its all-branch instruction bound."""
    instructions, starts = parse_text(disassembly), parse_starts(function_starts)
    require(start in starts and starts.index(start) + 1 < len(starts)
            and starts[starts.index(start) + 1] == end, 'exact successor interval missing')
    addresses = list(range(start, end, 4))
    require(start % 4 == end % 4 == 0 and start < end, 'invalid interval')
    require([a for a in sorted(instructions) if start <= a < end] == addresses,
            'incomplete function interval')
    edges = {a: successors(a, *instructions[a]) for a in addresses}
    require(all(b in edges for targets in edges.values() for b in targets),
            'branch/fallthrough escapes exact interval or targets a missing/misaligned word')
    reachable, pending = set(), [start]
    while pending:
        address = pending.pop()
        if address not in reachable:
            reachable.add(address)
            pending.extend(edges[address])
    require(reachable == set(addresses), 'unreachable interval word')
    degree = dict.fromkeys(addresses, 0)
    for targets in edges.values():
        for target in targets:
            degree[target] += 1
    pending, order = deque(a for a in addresses if degree[a] == 0), []
    while pending:
        address = pending.popleft()
        order.append(address)
        for target in edges[address]:
            degree[target] -= 1
            if degree[target] == 0:
                pending.append(target)
    require(len(order) == len(addresses), 'ordinary CFG cycle is unbounded here')
    lengths, following = {}, {}
    for address in reversed(order):
        if edges[address]:
            target = max(edges[address], key=lambda a: (lengths[a], -a))
            lengths[address], following[address] = 1 + lengths[target], target
        else:
            require(instructions[address] == ('ret', ''), 'non-RET terminal')
            lengths[address] = 1
    witness, address = [], start
    while True:
        witness.append(hex(address))
        if address not in following:
            break
        address = following[address]
    words = [{'address': hex(a), 'opcode': instructions[a][0], 'operands': instructions[a][1],
              'successors': [hex(b) for b in edges[a]], 'maximum_remaining_instruction_events': lengths[a]}
             for a in addresses]
    return {'interval_start': hex(start), 'interval_end_exclusive': hex(end),
            'decoded_words': len(addresses), 'reachable_words': len(reachable),
            'instructions_and_cfg': words,
            'normalized_words_sha256': sha(json.dumps(words, sort_keys=True, separators=(',', ':')).encode()),
            'ordinary_instruction_events_upper_bound': lengths[start],
            'longest_cfg_path': witness, 'path_is_not_claimed_data_feasible': True,
            'ordinary_explicit_sp_decrement_bytes': 0, 'ordinary_calls': 0,
            'all_ordinary_terminals_are_ret': True,
            'complete_physical_stack_bound_bytes': None, 'complete_work_bound': None,
            'elapsed_time_bound': None, 'exceptional_path_bound': None, 'caller_bound': None,
            'pointer_memory_bound': None}


def load_inputs(root, cache, guards):
    """Resolve original pins; later live bytes cannot become a fresh baseline."""
    documents = {name: object_json(pinned(cache / name, pin, guards)) for name, pin in PINS.items()}
    build, capture = documents['build/build-receipt.json'], documents['stack-v5/receipt.json']
    triage = documents['next-campaign-review-20261104T055051Z/review.json']
    require(build['source_stable'] is True and set(build['binaries']) == set(MODES), 'original build incomplete')
    require(capture['capture_passed'] is True and capture['failure'] is None
            and capture['input_changes'] == capture['original_capture_changes'] == [], 'original v5 capture refused')
    require(triage['passed'] == 47 and triage['failed'] == 113, 'historical failure inventory differs')
    pinned(Path(triage['historical_receipt']), triage['historical_receipt_sha256'], guards)
    sources = [root / 'crates/iroha_parliament_ballot_proof' / suffix for suffix in SOURCE_SUFFIXES]
    sources.append(cache / 'worker/generated_workspace.rs')
    for path in sources:
        require(str(path) in build['source_files'], 'targeted source absent from original build')
        pinned(path, build['source_files'][str(path)], guards)
    modes = {}
    for mode in MODES:
        report = documents[f'stack-v5/{mode}.json']
        identity, binding = report['identity'], report['image_binding']
        binary = build['binaries'][mode]
        require(binary['sha256'] == identity['binary_sha256'] == binding['binary_sha256']
                == capture['captures'][mode]['binary_sha256'], 'binary identity disagreement')
        require(identity['binary'] == binary['path'] == str(cache / 'binaries' / ('worker-' + mode)), 'binary path differs')
        require(capture['captures'][mode]['sha256'] == PINS[f'stack-v5/{mode}.json'], 'mode report identity differs')
        pinned(Path(binary['path']), binary['sha256'], guards)
        pinned(Path(identity['objdump']), identity['objdump_sha256'], guards)
        pinned(root / 'scripts/parliament_ballot_stack_audit.py', identity['script_sha256'], guards)
        require(identity['binary_tool_and_analyzer_unchanged_during_capture'] is True
                and identity['objdump_sha256'] == binding['objdump_sha256']
                and identity['script_sha256'] == binding['analyzer_sha256'], 'tool/analyzer identity differs')
        names = ('disassembly', 'starts', 'stubs', 'unwind')
        require(len(identity['commands']) == 4, 'raw capture command inventory differs')
        raw = {}
        flags = [('--disassemble', '--disassemble-zeroes', '--demangle', '--no-show-raw-insn'),
                 ('--macho', '--function-starts=both'), ('--macho', '--indirect-symbols'), ('--macho', '--unwind-info')]
        for name, command, options in zip(names, identity['commands'], flags):
            path = cache / 'stack-v5' / mode / (name + '.txt')
            require(command['output'] == str(path) and command['stderr'] == ''
                    and command['command'] == [identity['objdump'], *options, binary['path']], 'raw capture command differs')
            raw[name] = pinned(path, command['sha256'], guards).decode()
        for name, key in [('disassembly', 'disassembly_sha256'), ('starts', 'function_starts_sha256'), ('stubs', 'native_stubs_sha256')]:
            require(sha(raw[name].encode()) == binding[key], 'full image binding differs')
        require(len(report['roots']) == 5 and all(row['complete_stack_bound_bytes'] is None for row in report['roots']),
                'unexpected complete stack result')
        functions = [row for row in report['functions'] if row['address'] == hex(START)]
        require(len(functions) == 1 and functions[0]['end'] == hex(END)
                and functions[0]['calls'] == functions[0]['frame_errors'] == functions[0]['gaps'] == []
                and functions[0]['maximum_sp_decrement_bytes'] == 0
                and functions[0]['visited_instructions'] == 55, 'v5 leaf observation differs')
        modes[mode] = (raw, report)
    return modes, triage


def recheck(guards):
    """Re-read every original/reviewed input, preserving the original pins."""
    after = {path: sha(Path(path).read_bytes()) if Path(path).is_file() else None for path in guards}
    require(after == guards, 'input changed during analysis')
    return after


def write_json(path, value):
    """Create a new receipt, never overwrite an earlier attempt."""
    with path.open('x') as stream:
        stream.write(json.dumps(value, indent=2, sort_keys=True) + '\n')


def run(root, cache, output, helper_sha256, tests_sha256):
    """Write one new artifact-bound result or an explicit refused-attempt receipt."""
    root, cache, output = (Path(path).resolve() for path in (root, cache, output))
    require(not output.is_relative_to(root) and not cache.is_relative_to(output)
            and (not output.is_relative_to(cache)
                 or (output.parent == cache and output.name.startswith('record-leaf-'))),
            'output must be separate external evidence; cache outputs need a new record-leaf-* directory')
    output.mkdir(parents=True, exist_ok=False)
    report = {'schema': 'parliament-retained-record-leaf-v1', 'scope': SCOPE,
              'observed_utc': datetime.now(timezone.utc).isoformat(), 'analysis_passed': False,
              'failure': None, 'input_sha256_before': {}, 'input_sha256_after': {}, 'modes': {},
              'latest_source_build_validated': False, 'historical_evidence_rebound': False,
              'complete_stack_certificate': False, 'complete_workspace_certificate': False,
              'profile_qualified': False, 'release_qualified': False, 'physical_cost_calibrated': False,
              'no_build_disassembler_or_worker_execution': True,
              'premises': ['Exact retained binary, decoded words and function-start metadata are authoritative.',
                           'Code stays immutable; entry is the exact function start under the ordinary ARM64 ABI.',
                           'Profile/Role, borrowed slice and output storage satisfy the compiled Rust ABI and validity requirements.',
                           'Loads/stores through caller pointers may access caller stack; zero explicit SP decrement is not zero stack or memory use.',
                           'Ordinary instructions complete; faults, traps, hidden unwind, signals and reentry are unbounded here.',
                           'The CFG counts instruction events, including branches and RET, not cycles or elapsed time.']}
    guards = report['input_sha256_before']
    try:
        helper = pinned(Path(__file__).resolve(), helper_sha256, guards)
        tests = pinned(root / 'scripts/tests/parliament_ballot_retained_record_bound_test.py', tests_sha256, guards)
        (output / 'helper.py.template').write_bytes(helper)
        (output / 'tests.py.template').write_bytes(tests)
        modes, triage = load_inputs(root, cache, guards)
        report['historical_selected_nodes'] = {'passed': 47, 'failed': 113, 'rerun': False,
                                               'receipt': triage['historical_receipt'],
                                               'receipt_sha256': triage['historical_receipt_sha256']}
        report['prior_observed_build_input_drift'] = triage['build_inputs_different_from_retained_capture']
        report['live_source_scope'] = 'Only the nine targeted source files are checked; full build/source freshness is not established.'
        for mode, (raw, original) in modes.items():
            leaf = leaf_bound(raw['disassembly'], raw['starts'])
            require(leaf['decoded_words'] == 55, 'unexpected retained leaf size')
            report['modes'][mode] = {'leaf': leaf, 'original_v5_report_sha256': PINS[f'stack-v5/{mode}.json'],
                                     'original_image_binding': original['image_binding'],
                                     'inherited_outer_roots_unchanged': original['roots']}
        report['input_sha256_after'] = recheck(guards)
        report['analysis_passed'] = True
    except (OSError, ValueError, KeyError, TypeError, IndexError) as error:
        report['failure'] = type(error).__name__ + ': ' + str(error)
    write_json(output / 'report.json', report)
    return report


def main():
    """Expose explicit paths and reviewed hashes; no implicit pin refresh."""
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('root', 'cache', 'output'):
        parser.add_argument('--' + name, type=Path, required=True)
    parser.add_argument('--helper-sha256', required=True)
    parser.add_argument('--tests-sha256', required=True)
    args = parser.parse_args()
    report = run(args.root, args.cache, args.output, args.helper_sha256, args.tests_sha256)
    print(json.dumps({'analysis_passed': report['analysis_passed'], 'failure': report['failure'],
                      'report': str(args.output.resolve() / 'report.json')}))
    return 0 if report['analysis_passed'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
