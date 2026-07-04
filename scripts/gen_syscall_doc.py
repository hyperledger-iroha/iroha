#!/usr/bin/env python3
import re
import sys
from pathlib import Path

SRC = Path('crates/ivm_abi/src/syscalls.rs')
if not SRC.exists():
    print(f'Error: {SRC} not found', file=sys.stderr)
    sys.exit(1)

print('# Generated IVM Syscall Table')
print('\nThis file is generated from `crates/ivm_abi/src/syscalls.rs`. Edit the source to change syscall numbers; then re-run this script.\n')
print('| Name | Value (hex) | Note |')
print('|------|-------------|------|')

pattern = re.compile(r'^pub const (SYSCALL_[A-Za-z0-9_]+): u32 = ([^;]+);')
for line in SRC.read_text(encoding='utf-8').splitlines():
    m = pattern.match(line)
    if not m:
        continue
    name, rhs = m.group(1), m.group(2).strip()
    note = ''
    valhex = ''
    if rhs.lower().startswith('0x'):
        valhex = f'0x{int(rhs.replace("_", ""), 16):X}'
    elif rhs.isdigit():
        valhex = f'0x{int(rhs):X}'
    elif rhs.startswith('SYSCALL_'):
        note = f'alias of {rhs}'
    else:
        note = rhs
    if name.startswith('SYSCALL_KOTO_TEST_'):
        note = 'host-private test helper; outside ABI'
    print(f'| {name} | {valhex} | {note} |')

print('\nNote: This table lists syscall constants. The contract ABI allowlist is enforced by `abi_syscall_list()`; host-private test helpers are outside that ABI.')
