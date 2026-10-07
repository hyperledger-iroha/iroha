#!/usr/bin/env python3
"""Run the independent full PIPA reference with only Python's standard library.

It verifies genuine frozen full proofs, never calls Rust/native code, and keeps
historical oracle framing explicit and separate from production verification.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from reference_verifier import InvalidProof, require
from reference_verifier.verify import verify, verify_captured_oracle


def expected_names():
    """The exact supported fixture matrix, independent of the supplied JSON."""
    names = set()
    for curve in ('ep', 'eq'):
        for profile in ('blake', 'poseidon'):
            for k in (6, 8):
                names.add(f'{curve}/oracle/{profile}/{k}')
            for mode in ('Committed', 'Direct'):
                for compression in ('false', 'true'):
                    names.add(f'{curve}/v1/{profile}/{mode}/{compression}')
                names.add(f'{curve}/v2/{profile}/{mode}/field')
        names.add(f'{curve}/v2/base/Direct/field')
        names.add(f'{curve}/v2/base/k6/bits4')
        for k in range(6, 11):
            names.add(f'{curve}/v2/base/k{k}/bounded-or-field')
    return names


def check(document):
    """Verify every exact case and enforce the complete positive matrix."""
    require(document['format'] == 'iroha.pipa.reference.v1', 'reference fixture format')
    cases = document['cases']
    require(isinstance(cases, list) and len(cases) == len(expected_names()), 'reference case count')
    require({case['name'] for case in cases} == expected_names(), 'reference case identities')
    parameters = document['parameters']
    require(set(parameters) == {f'{curve}/{k}' for curve in ('ep', 'eq') for k in range(6, 11)},
            'reference parameter matrix')
    for case in cases:
        require(type(case['descriptor_version']) is int and type(case['k']) is int,
                'canonical reference metadata')
        proof = bytes.fromhex(case['proof'])
        require(hashlib.sha256(proof).hexdigest() == case['proof_sha256'], 'captured proof digest')
        common = dict(descriptor=bytes.fromhex(case['descriptor']), key=bytes.fromhex(case['verifying_key']),
                      parameter_bytes=bytes.fromhex(parameters[f"{case['curve']}/{case['k']}"]),
                      instances=[[int.from_bytes(bytes.fromhex(value), 'little') for value in column]
                                 for column in case['instances']], proof=proof)
        if '/oracle/' in case['name']:
            require(case['descriptor_version'] == 1 and 'oracle_repr' in case, 'explicit oracle case')
            verify_captured_oracle(**common, transcript_repr=int.from_bytes(bytes.fromhex(case['oracle_repr']), 'little'))
        else:
            require('oracle_repr' not in case, 'no production transcript override')
            verify(**common, version=case['descriptor_version'])
    return len(cases)


def main():
    """Read one finite fixture and report only after all complete decisions pass."""
    path = Path(sys.argv[1]) if len(sys.argv) == 2 else Path(__file__).with_name('reference_v1.json')
    require(len(sys.argv) <= 2, 'one optional fixture path')
    require(path.stat().st_size <= 16 * 1024 * 1024, 'reference fixture bound')
    count = check(json.loads(path.read_text()))
    print(f'{path}: verified full_proofs={count}, pinned_parameter_sets=10, curves=2, profiles=3')


if __name__ == '__main__':
    try:
        main()
    except (InvalidProof, KeyError, TypeError, ValueError, OSError) as error:
        print(f'reference verifier rejected: {error}', file=sys.stderr)
        raise SystemExit(1) from error
