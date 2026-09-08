#!/usr/bin/env python3
"""Independent adaptive-context insertion controls, not a theorem proof."""
import argparse
import hashlib
import json
from pathlib import Path
import random

ROOT = Path(__file__).resolve().parents[2]
C = 61
CONTEXTS = ("false-0", "false-1", "true")
FALSE = frozenset(CONTEXTS[:2])
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
        if x[0] == 'G' and len(x) == 4 and x[2] in CONTEXTS:
            short.add(x[3])
        elif x[:2] == ('H', 'chain') and x[2] in CONTEXTS:
            short.add(x[5])
            tapes[x[3]].add(x[4])
        elif x[:2] == ('H', 'parent') and x[2] in CONTEXTS:
            short.update(x[4:6])
    return short, tapes


def tree(db, context, j, root):
    x = invert(db, 0, root)
    if x is None or x[:4] != ('H', 'parent', context, j):
        return (None, None)
    values = []
    for position, child in enumerate(x[4:6]):
        leaf = invert(db, 0, child)
        if leaf is None or leaf[:5] != ('H', 'leaf', context, j, position):
            values.append(None)
        else:
            values.append(leaf[5])
    return tuple(values)


def extract(db, context, r, sigma):
    if r == 0:
        return () if sigma == ANCHOR else None
    chain = invert(db, 0, sigma)
    if chain is None or chain[:4] != ('H', 'chain', context, r):
        return None
    _, _, _, _, tau, rt = chain
    g = invert(db, r, tau)
    if g is None or g[:3] != ('G', r, context):
        return None
    earlier = extract(db, context, r - 1, g[3])
    if earlier is None:
        return None
    # The dummy first tape has four possible raw values but no message bits.
    message = None if r == 1 else tau & 1
    return earlier + ((message, tree(db, context, r, rt)),)


def state_before(context, transcript):
    bad = False
    for j, (message, _) in enumerate(transcript, start=1):
        if j == 2:
            bad |= message == ((transcript[0][1][0] or 0) ^ int(context == "false-1"))
    return bad


def bad_transition(context, transcript, j, tau):
    if state_before(context, transcript) or j == 1:
        return False
    word = transcript[j - 2][1]
    index = 0 if j == 2 else 1
    return (tau & 1) == ((word[index] or 0) ^ int(context == "false-1"))


def property_r(db):
    if collision(db):
        return True
    for x, tau in db.items():
        if x[0] != 'G' or x[2] not in FALSE:
            continue
        j, sigma = x[1], x[3]
        transcript = extract(db, x[2], j - 1, sigma)
        if transcript is not None and bad_transition(x[2], transcript, j, tau):
            return True
    return False


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,default=ROOT/"target/fastpq-production-validation/compact-adaptive-context-controls.json")
    args=parser.parse_args()
    # A single database spans two false contexts and one true context. H
    # outputs are globally distinct; G outputs are distinct within each type.
    complete = {}
    for index, context in enumerate(CONTEXTS):
        base = 8 * index
        complete.update({
            ('G', 1, context, 0): index,
            ('H', 'leaf', context, 1, 0, 1): base + 1,
            ('H', 'leaf', context, 1, 1, 0): base + 2,
            ('H', 'parent', context, 1, base + 1, base + 2): base + 3,
            ('H', 'chain', context, 1, index, base + 3): base + 4,
            ('G', 2, context, base + 4): index,
            ('H', 'leaf', context, 2, 0, 0): base + 5,
            ('H', 'leaf', context, 2, 1, 1): base + 6,
            ('H', 'parent', context, 2, base + 5, base + 6): base + 7,
            ('H', 'chain', context, 2, index, base + 7): base + 8,
            ('G', 3, context, base + 8): (index + 1) % TAPES,
        })
    assert not collision(complete)
    for index, context in enumerate(CONTEXTS):
        assert extract(complete, context, 2, 8 * index + 8) is not None
        for other in CONTEXTS:
            if other != context:
                assert extract(complete, other, 2, 8 * index + 8) is None
    assert property_r(complete)
    true_only = {x: y for x, y in complete.items() if x[2] == 'true'}
    assert not property_r(true_only)
    rng = random.Random(0xADA971)
    entries = list(complete.items())
    masks = {0, (1 << len(entries)) - 1}
    masks.update(((1 << len(entries)) - 1) ^ (1 << i) for i in range(len(entries)))
    masks.update(rng.randrange(1 << len(entries)) for _ in range(512))
    additions = [
        ('H', 'chain', 'false-0', 1, 1, 3),
        ('H', 'chain', 'false-1', 2, 0, 15),
        ('H', 'parent', 'false-1', 1, 2, 1),
        ('H', 'malformed', 'opaque'),
        ('G', 1, 'wrong-context', 0),
        ('G', 2, 'false-0', 12),
        ('G', 3, 'false-1', 8),
    ]
    tested = stability_changes = membership_flips = 0
    for mask in sorted(masks):
        db = {x: y for i, (x, y) in enumerate(entries) if mask & (1 << i)}
        assert not collision(db)
        sh, sg = pointers(db)
        original_r = property_r(db)
        endpoints = sorted({ANCHOR, 4, 8, 12, 16, rng.randrange(C)})
        before = {(context, r, sigma): extract(db, context, r, sigma) for context in CONTEXTS for r in range(K + 1) for sigma in endpoints}
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
                    for (context, r, sigma), previous in before.items():
                        now = extract(updated, context, r, sigma)
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
                    assert not original_r and x[2] in FALSE
                    prior = extract(db, x[2], ty - 1, x[3])
                    assert prior is not None and bad_transition(x[2], prior, ty, y), (db, x, y)
    result = {'kind': 'independent adaptive-context insertion counterexample search, not a theorem proof',
              'status': 'pass', 'contexts': list(CONTEXTS), 'database_subsets': len(masks), 'insertions': tested,
              'extraction_changes': stability_changes, 'membership_flips': membership_flips,
              'source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    result["specification_sha256"]=hashlib.sha256((ROOT/"specs/fastpq_compact_adaptive_context.md").read_bytes()).hexdigest()
    result["output"]=str(args.output)
    args.output.parent.mkdir(parents=True,exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    if not __debug__:
        raise SystemExit('Do not disable assertions in the adaptive-context controls.')
    main()
