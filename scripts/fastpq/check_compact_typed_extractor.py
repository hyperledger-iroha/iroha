#!/usr/bin/env python3
"""Independent bounded extractor counterexample search, not a theorem proof."""
import argparse
import hashlib
import json
from pathlib import Path
import random

ROOT = Path(__file__).resolve().parents[2]
C = 31
TAPES = 4
ANCHOR = 0
K = 2


def collision(db):
    outputs = {}
    for x, y in db.items():
        ty = 0 if x[0] == 'H' else x[1]
        if (ty, y) in outputs:
            return True
        outputs[ty, y] = x
    return False


def invert(db, ty, value):
    found = [x for x, y in db.items() if y == value and (0 if x[0] == 'H' else x[1]) == ty]
    return found[0] if len(found) == 1 else None


def pointers(db):
    short, tapes = set(), {j: set() for j in range(1, K + 2)}
    for x in db:
        if x[0] == 'G' and len(x) == 4 and x[2] == 'c':
            short.add(x[3])
        elif x[:2] == ('H', 'chain') and x[2] == 'c':
            short.add(x[5])
            tapes[x[3]].add(x[4])
        elif x[:2] == ('H', 'parent') and x[2] == 'c':
            short.update(x[4:6])
    return short, tapes


def tree(db, j, root):
    x = invert(db, 0, root)
    if x is None or x[:4] != ('H', 'parent', 'c', j):
        return (None, None)
    values = []
    for position, child in enumerate(x[4:6]):
        leaf = invert(db, 0, child)
        if leaf is None or leaf[:5] != ('H', 'leaf', 'c', j, position):
            values.append(None)
        else:
            values.append(leaf[5])
    return tuple(values)


def extract(db, r, sigma):
    if r == 0:
        return () if sigma == ANCHOR else None
    chain = invert(db, 0, sigma)
    if chain is None or chain[:4] != ('H', 'chain', 'c', r):
        return None
    _, _, _, _, tau, rt = chain
    g = invert(db, r, tau)
    if g is None or g[:3] != ('G', r, 'c'):
        return None
    earlier = extract(db, r - 1, g[3])
    if earlier is None:
        return None
    # The dummy first tape has four possible raw values but no message bits.
    message = None if r == 1 else tau & 1
    return earlier + ((message, tree(db, r, rt)),)


def state_before(transcript):
    bad = False
    for j, (message, _) in enumerate(transcript, start=1):
        if j == 2:
            bad |= message == (transcript[0][1][0] or 0)
    return bad


def bad_transition(transcript, j, tau):
    if state_before(transcript) or j == 1:
        return False
    word = transcript[j - 2][1]
    index = 0 if j == 2 else 1
    return (tau & 1) == (word[index] or 0)


def property_r(db):
    if collision(db):
        return True
    for x, tau in db.items():
        if x[0] != 'G' or x[2] != 'c':
            continue
        j, sigma = x[1], x[3]
        transcript = extract(db, j - 1, sigma)
        if transcript is not None and bad_transition(transcript, j, tau):
            return True
    return False


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,default=ROOT/"target/fastpq-production-validation/compact-typed-extractor-controls.json")
    args=parser.parse_args()
    # Two complete rounds, distinct H outputs, full raw tape fields and a final G endpoint.
    complete = {
        ('G', 1, 'c', 0): 3,
        ('H', 'leaf', 'c', 1, 0, 1): 1,
        ('H', 'leaf', 'c', 1, 1, 0): 2,
        ('H', 'parent', 'c', 1, 1, 2): 3,
        ('H', 'chain', 'c', 1, 3, 3): 4,
        ('G', 2, 'c', 4): 2,
        ('H', 'leaf', 'c', 2, 0, 0): 5,
        ('H', 'leaf', 'c', 2, 1, 1): 6,
        ('H', 'parent', 'c', 2, 5, 6): 7,
        ('H', 'chain', 'c', 2, 2, 7): 8,
        ('G', 3, 'c', 8): 1,
    }
    assert extract(complete, 2, 8) == ((None, (1, 0)), (0, (0, 1)))
    assert property_r(complete)
    # Same decoded first message, different complete tape: extraction must fail.
    forged = dict(complete)
    forged[('G', 1, 'c', 0)] = 1
    assert extract(forged, 2, 8) is None
    rng = random.Random(0xFCBC5)
    entries = list(complete.items())
    masks = {0, (1 << len(entries)) - 1}
    masks.update(((1 << len(entries)) - 1) ^ (1 << i) for i in range(len(entries)))
    masks.update(rng.randrange(1 << len(entries)) for _ in range(512))
    additions = [
        ('H', 'chain', 'c', 1, 1, 3),
        ('H', 'chain', 'c', 2, 0, 7),
        ('H', 'parent', 'c', 1, 2, 1),
        ('H', 'malformed', 'opaque'),
        ('G', 1, 'wrong-context', 0),
        ('G', 2, 'c', 0),
        ('G', 3, 'c', 4),
    ]
    tested = stability_changes = membership_flips = 0
    for mask in sorted(masks):
        db = {x: y for i, (x, y) in enumerate(entries) if mask & (1 << i)}
        assert not collision(db)
        sh, sg = pointers(db)
        original_r = property_r(db)
        endpoints = sorted({ANCHOR, 4, 8, rng.randrange(C)})
        before = {(r, sigma): extract(db, r, sigma) for r in range(K + 1) for sigma in endpoints}
        for x in [x for x, _ in entries] + additions:
            if x in db:
                continue
            ty = 0 if x[0] == 'H' else x[1]
            alphabet = range(C if ty == 0 else TAPES)
            image = {y for z, y in db.items() if (0 if z[0] == 'H' else z[1]) == ty}
            for y in alphabet:
                updated = dict(db)
                updated[x] = y
                tested += 1
                if not collision(updated):
                    for (r, sigma), previous in before.items():
                        now = extract(updated, r, sigma)
                        if now != previous:
                            stability_changes += 1
                            assert y in (sh | {sigma} if ty == 0 else sg[ty]), (db, x, y, r, sigma)
                if property_r(updated) == original_r:
                    continue
                membership_flips += 1
                if ty == 0:
                    assert y in sh | image, (db, x, y)
                elif y not in sg[ty] | image:
                    # A non-exceptional flip must be creation by the new G endpoint itself.
                    assert not original_r and x[2] == 'c'
                    prior = extract(db, ty - 1, x[3])
                    assert prior is not None and bad_transition(prior, ty, y), (db, x, y)
    result = {'kind': 'independent finite extractor counterexample search, not a theorem proof',
              'status': 'pass', 'database_subsets': len(masks), 'insertions': tested,
              'extraction_changes': stability_changes, 'membership_flips': membership_flips,
              'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    result["specification_sha256"]=hashlib.sha256((ROOT/"specs/fastpq_compact_typed_compiler.md").read_bytes()).hexdigest()
    result["output"]=str(args.output)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    if not __debug__:
        raise SystemExit('Do not disable assertions in the extractor review controls.')
    main()
