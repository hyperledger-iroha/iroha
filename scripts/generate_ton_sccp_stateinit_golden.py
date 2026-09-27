#!/usr/bin/env python3
"""Generate or check `fixtures/sccp/ton_stateinit_v1.json` (specs/sccp.md §5.3.1, §11).

Purpose
    Pins the canonical initial data and workchain-0 address of the SCCP v1 TON
    minter for fixed NetworkId, generation roster, revision and cap, together
    with the code cell hashes and depths of the minter, wallet and bucket. Taira
    (`RegisterRoute`) and the Rust `iroha_sccp::v1::ton_cell` builder must
    reproduce these values from code hashes and depths alone.

Method
    1. Build the contracts with the pinned Acton 1.2.0 (`ton_sccp_builder.py`).
    2. Parse each compiled code BoC and recompute every value independently in
       Python (cells, roster digest, initial data, StateInit, child addresses).
    3. Run `contracts/ton/sccp/scripts/stateinit-golden.tolk`, which computes
       the same values with the contracts' own Tolk types, and require exact
       agreement.
    4. Write the canonical JSON, or with `--check` require the committed file
       to be byte-identical.

Options
    --check     compare instead of writing (non-zero exit on any difference)
    --no-build  reuse existing `build/` outputs
    --acton / --offline  as in `ton_sccp_builder.py`

Requires Python 3.9+ and the standard library only.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple

sys.path.insert(0, str(Path(__file__).resolve().parent))
import ton_sccp_builder as builder  # noqa: E402

ROOT = builder.ROOT
FIXTURE = ROOT / "fixtures" / "sccp" / "ton_stateinit_v1.json"
SCRIPT = "scripts/stateinit-golden.tolk"
SCHEMA = "iroha.sccp.ton-stateinit.v1"
NETWORK_ID = int("11" * 32, 16)
WALLET_OWNER = int("ab" * 32, 16)


class GoldenError(RuntimeError):
    """A mismatch or malformed input."""


def golden_member(index: int) -> int:
    """Member `i` of a golden roster: 20 bytes of value `i`."""

    return int.from_bytes(bytes([index]) * 20, "big")


VECTORS: Tuple[Dict[str, Any], ...] = (
    {
        "label": "n4",
        "route_revision": 1,
        "max_supply": 10**18,
        "generation": 7,
        "valid_from_ms": 1_800_000_000_000,
        "valid_until_ms": 1_801_209_600_000,
        "members": [golden_member(i) for i in range(1, 5)],
    },
    {
        "label": "n31",
        "route_revision": 2,
        "max_supply": (1 << 96) - 1,
        "generation": 12,
        "valid_from_ms": 1_800_000_000_000,
        "valid_until_ms": 1_801_209_600_000,
        "members": [0, 0, 0] + [golden_member(i) for i in range(1, 29)],
    },
)


def _hex(value: bytes) -> str:
    return value.hex()


def _cell_record(cell: builder.Cell, refs: Sequence[str]) -> Dict[str, Any]:
    return {
        "bit_len": len(cell.bits),
        "data": cell.data_hex(),
        "hash": _hex(cell.hash),
        "depth": cell.depth,
        "refs": list(refs),
    }


def wallet_address(owner: int, minter: int, wallet_code: builder.Cell) -> int:
    """Account id of the wallet of `0:owner` under minter `0:minter`."""

    data = builder.Builder().coins(0).address(0, owner).address(0, minter).end()
    return int.from_bytes(builder.state_init(wallet_code, data).hash, "big")


def bucket_address(minter: int, index: int, bucket_code: builder.Cell) -> int:
    """Account id of bucket `index` (flags clear) under minter `0:minter`."""

    data = builder.Builder().address(0, minter).uint(index, 64).uint(0, 256).uint(0, 256).end()
    return int.from_bytes(builder.state_init(bucket_code, data).hash, "big")


def compute(project: Path = builder.PROJECT) -> Dict[str, Any]:
    """Recomputes the fixture from the compiled code."""

    code = {
        "minter": builder.load_compiled_code("SccpTairaXorMinter", project),
        "wallet": builder.load_compiled_code("SccpTairaXorWallet", project),
        "bucket": builder.load_compiled_code("SccpConsumedBucket", project),
    }
    code_records = {}
    for name, cell in code.items():
        cells, bits = builder.cell_tree_size(cell)
        code_records[name] = {"hash": _hex(cell.hash), "depth": cell.depth, "cells": cells, "bits": bits}
    vectors = []
    for spec in VECTORS:
        members = spec["members"]
        n = len(members)
        cells = builder.minter_initial_data(
            NETWORK_ID,
            spec["route_revision"],
            spec["max_supply"],
            spec["generation"],
            spec["valid_from_ms"],
            spec["valid_until_ms"],
            members,
            code["wallet"],
            code["bucket"],
        )
        member_cells: List[builder.Cell] = []
        chunk: Optional[builder.Cell] = cells["members"]
        while chunk is not None:
            member_cells.append(chunk)
            chunk = chunk.refs[0] if chunk.refs else None
        root = cells["root"]
        state_init = builder.state_init(code["minter"], root)
        account = int.from_bytes(state_init.hash, "big")
        vectors.append(
            {
                "label": spec["label"],
                "taira_network_id": f"{NETWORK_ID:064x}",
                "route_revision": spec["route_revision"],
                "max_supply": str(spec["max_supply"]),
                "generation": spec["generation"],
                "valid_from_ms": spec["valid_from_ms"],
                "valid_until_ms": spec["valid_until_ms"],
                "n": n,
                "t": builder.roster_threshold(n),
                "members": [f"{member:040x}" for member in members],
                "roster_digest": f"{builder.roster_digest(NETWORK_ID, spec['generation'], spec['valid_from_ms'], spec['valid_until_ms'], members):064x}",
                "initial_data": {
                    "hash": _hex(root.hash),
                    "depth": root.depth,
                    "root": _cell_record(root, ["config", "roster"]),
                    "config": _cell_record(cells["config"], ["wallet_code", "bucket_code"]),
                    "roster": _cell_record(cells["roster"], ["members[0]"]),
                    "members": [
                        _cell_record(cell, [f"members[{i + 1}]"] if cell.refs else [])
                        for i, cell in enumerate(member_cells)
                    ],
                },
                "state_init_hash": _hex(state_init.hash),
                "address": {
                    "workchain": 0,
                    "account_id": f"{account:064x}",
                    "raw": f"0:{account:064x}",
                },
                "children": {
                    "wallet_owner": f"0:{WALLET_OWNER:064x}",
                    "wallet_account_id": f"{wallet_address(WALLET_OWNER, account, code['wallet']):064x}",
                    "bucket_0_account_id": f"{bucket_address(account, 0, code['bucket']):064x}",
                    "bucket_1_account_id": f"{bucket_address(account, 1, code['bucket']):064x}",
                },
            }
        )
    return {
        "schema": SCHEMA,
        "spec": "specs/sccp.md §5.3.1 (revision 3)",
        "generator": "scripts/generate_ton_sccp_stateinit_golden.py",
        "toolchain": {"acton": builder.ACTON_VERSION, "tolk": builder.TOLK_VERSION},
        "code": code_records,
        "vectors": vectors,
    }


def parse_script_output(output: str) -> Dict[Tuple[str, str], str]:
    """Parses `sccp-stateinit <label> <key>=<value>` lines."""

    values: Dict[Tuple[str, str], str] = {}
    for line in output.splitlines():
        parts = line.strip().split()
        if len(parts) != 3 or parts[0] != "sccp-stateinit" or "=" not in parts[2]:
            continue
        key, value = parts[2].split("=", 1)
        if (parts[1], key) in values:
            raise GoldenError(f"duplicate script value {parts[1]}.{key}")
        values[(parts[1], key)] = value
    return values


def cross_check(fixture: Dict[str, Any], tolk: Dict[Tuple[str, str], str]) -> None:
    """Requires the Tolk-computed values to equal the Python ones."""

    expected: Dict[Tuple[str, str], str] = {}
    for name, record in fixture["code"].items():
        expected[("code", f"{name}_code_hash")] = record["hash"]
        expected[("code", f"{name}_code_depth")] = str(record["depth"])
    for vector in fixture["vectors"]:
        label = vector["label"]
        expected[(label, "roster_digest")] = vector["roster_digest"]
        expected[(label, "data_hash")] = vector["initial_data"]["hash"]
        expected[(label, "data_depth")] = str(vector["initial_data"]["depth"])
        expected[(label, "state_init_hash")] = vector["state_init_hash"]
        expected[(label, "workchain")] = "0"
        expected[(label, "account_id")] = vector["address"]["account_id"]
        expected[(label, "wallet_of_abab")] = vector["children"]["wallet_account_id"]
        expected[(label, "bucket_0")] = vector["children"]["bucket_0_account_id"]
        expected[(label, "bucket_1")] = vector["children"]["bucket_1_account_id"]
    if set(tolk) != set(expected):
        raise GoldenError(f"script keys differ: {sorted(set(tolk) ^ set(expected))}")
    for key, value in expected.items():
        if tolk[key] != value:
            raise GoldenError(f"{key[0]}.{key[1]}: Tolk {tolk[key]} != Python {value}")


def render(fixture: Dict[str, Any]) -> str:
    """Canonical JSON text."""

    return json.dumps(fixture, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def run(acton: Path, check: bool, build: bool = True) -> None:
    """Builds (optionally), cross-checks and writes or checks the fixture."""

    if build:
        builder.run_acton(acton, ["build"])
    builder.check_tolk_stdlib()
    fixture = compute()
    cross_check(fixture, parse_script_output(builder.run_acton(acton, ["script", SCRIPT])))
    text = render(fixture)
    if check:
        if not FIXTURE.is_file() or FIXTURE.read_text(encoding="utf-8") != text:
            raise GoldenError(f"{FIXTURE.relative_to(ROOT)} is stale; rerun without --check")
        print(f"{FIXTURE.relative_to(ROOT)} matches the Tolk and Python StateInit computations")
        return
    FIXTURE.parent.mkdir(parents=True, exist_ok=True)
    FIXTURE.write_text(text, encoding="utf-8")
    print(f"wrote {FIXTURE.relative_to(ROOT)}")


def main(arguments: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="compare instead of writing")
    parser.add_argument("--no-build", action="store_true", help="reuse existing build outputs")
    parser.add_argument("--acton", help="absolute Acton executable (exact pinned version)")
    parser.add_argument("--offline", action="store_true", help="never download Acton")
    parsed = parser.parse_args(arguments)
    try:
        acton = builder.resolve_acton(parsed.acton, parsed.offline)
        run(acton, check=parsed.check, build=not parsed.no_build)
        return 0
    except (GoldenError, builder.TonBuilderError) as error:
        print(f"TON SCCP StateInit golden failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
