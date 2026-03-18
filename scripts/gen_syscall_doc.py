#!/usr/bin/env python3
import re
import sys
from pathlib import Path

SRC = Path('crates/ivm/src/syscalls.rs')
if not SRC.exists():
    print(f'Error: {SRC} not found', file=sys.stderr)
    sys.exit(1)

print('# Generated IVM Syscall Table')
print('\nThis file is generated from `crates/ivm/src/syscalls.rs`. Edit the source to change syscall numbers; then re-run this script.\n')
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
    if rhs.startswith('0x'):
        valhex = rhs.upper()
    elif rhs.isdigit():
        valhex = f'0x{int(rhs):X}'
    elif rhs.startswith('SYSCALL_'):
        note = f'alias of {rhs}'
    else:
        note = rhs
    print(f'| {name} | {valhex} | {note} |')

print('\nNote: Aliases resolve to the value of their target constant at compile time.')

