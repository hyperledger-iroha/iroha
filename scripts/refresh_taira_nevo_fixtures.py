#!/usr/bin/env python3
"""Refresh NEVO golden composition from the current Taira base and owned overlay.

This maintains public test fixtures only. It neither signs genesis nor produces
operator approval. Kagami's native review validator remains authoritative.
"""
import argparse
import hashlib
import json
from pathlib import Path

FIXTURES = Path('crates/iroha_kagami/tests/fixtures/taira_nevo_v2')
TAIRA = Path('configs/soranexus/taira')


def expected_fixtures(root: Path) -> dict[Path, bytes]:
    """Compose exactly one overlay and refresh its byte-bound review digests."""
    base_bytes = (root / TAIRA / 'genesis.template.json').read_bytes()
    config_bytes = (root / TAIRA / 'config.toml').read_bytes()
    base = json.loads(base_bytes)
    overlay = json.loads((root / TAIRA / 'nevo_genesis_overlay.template.json').read_bytes())
    if (set(overlay) != {'instructions', 'ivm_triggers', 'topology'}
            or type(overlay['instructions']) is not list or len(overlay['instructions']) != 29
            or overlay['ivm_triggers'] != [] or overlay['topology'] != []):
        raise ValueError('NEVO overlay must be exactly one instruction-only transaction with 29 instructions')
    if base['chain'] != 'fc56984b-2be7-431d-840e-21514d1883f0' or base['chain_discriminant'] != 369:
        raise ValueError('NEVO fixtures require the canonical Taira chain')
    review = json.loads((root / FIXTURES / 'review.json').read_bytes())
    public = json.loads((root / FIXTURES / 'public-inputs.json').read_bytes())
    canonical_public = (json.dumps(public, ensure_ascii=False, sort_keys=True, separators=(',', ':')) + '\n').encode()
    if review['public_inputs_sha256'] != hashlib.sha256(canonical_public).hexdigest():
        raise ValueError('public input changes require an explicit review update')
    base['transactions'].append(overlay)
    genesis = (json.dumps(base, ensure_ascii=False, indent=2) + '\n').encode()
    review['base_genesis_sha256'] = hashlib.sha256(base_bytes).hexdigest()
    review['base_config_sha256'] = hashlib.sha256(config_bytes).hexdigest()
    review['unsigned_genesis_sha256'] = hashlib.sha256(genesis).hexdigest()
    return {
        FIXTURES / 'unsigned-genesis.template.json': genesis,
        FIXTURES / 'review.json': (json.dumps(review, ensure_ascii=False, indent=2) + '\n').encode(),
    }


def main() -> int:
    """Check by default; regenerate only when explicitly asked to write."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true', help='refresh the two generated public fixtures')
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    expected = expected_fixtures(root)
    changed = [relative for relative, data in expected.items() if (root / relative).read_bytes() != data]
    if args.write:
        for relative in changed:
            (root / relative).write_bytes(expected[relative])
    for relative in changed:
        print(f'{"refreshed" if args.write else "stale"}: {relative}')
    return int(bool(changed) and not args.write)


if __name__ == '__main__':
    raise SystemExit(main())
