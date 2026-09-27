#!/usr/bin/env python3
"""Regenerate exact length/SHA-256 pins for the maintained Torii contract asset."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
ASSET = Path("crates/iroha_torii/src/openapi/tests/openapi_contracts_v1.json")
CONSUMER = Path("crates/iroha_torii/src/openapi/tests/sorafs_contracts.rs")


def update_source(payload: bytes, source: str) -> str:
    """Validate the complete ordered inventory before deriving its two scalar pins."""
    value = json.loads(payload)
    version = int(re.search(r"OPENAPI_CONTRACT_ASSET_VERSION: u64 = (\d+);", source).group(1))
    section_source = source.split("const OPENAPI_CONTRACT_SECTION_ORDER:", 1)[1].split("];", 1)[0]
    expected_sections = re.findall(r'"([^"\n]+)"', section_source)
    if set(value) != {"version", "sections"} or value["version"] != version:
        raise ValueError("contract asset envelope/version differs")
    sections = value["sections"]
    if not isinstance(sections, list) or [section.get("id") for section in sections] != expected_sections:
        raise ValueError("contract asset section inventory differs")
    for section in sections:
        if set(section) != {"id", "values"} or not isinstance(section["values"], list):
            raise ValueError("contract asset section shape differs")
        if any(not isinstance(word, str) or not word for word in section["values"]):
            raise ValueError("contract asset inventory contains a non-string or empty value")
    length = f"{len(payload):_}"
    digest = hashlib.sha256(payload).hexdigest()
    updated, lengths = re.subn(r"(OPENAPI_CONTRACT_ASSET_LEN: usize = )[\d_]+;", rf"\g<1>{length};", source)
    updated, hashes = re.subn(r'(OPENAPI_CONTRACT_ASSET_SHA256: &str =\s*)"[0-9a-f]{64}";', rf'\g<1>"{digest}";', updated)
    if lengths != 1 or hashes != 1:
        raise ValueError("expected exactly one length and digest pin")
    return updated


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail on drift without changing the Rust consumer")
    args = parser.parse_args()
    path = ROOT / CONSUMER
    current = path.read_text()
    updated = update_source((ROOT / ASSET).read_bytes(), current)
    if args.check:
        if updated != current:
            raise SystemExit("Torii OpenAPI contract pins differ; run scripts/update_torii_openapi_contract_pins.py")
    elif updated != current:
        path.write_text(updated)
    print("Torii OpenAPI contract length and SHA-256 pins match the exact asset.")


if __name__ == "__main__":
    main()
