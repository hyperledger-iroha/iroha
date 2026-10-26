"""Bounded source-bound PIPA-AS transcript schedules, never a fold verifier."""

from functools import lru_cache
import hashlib
import importlib
import importlib.util
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
REFERENCE = ROOT / 'fixtures/native_prover/reference_verifier'
FILES = frozenset(('README.md', 'transcript.py', 'test_transcript.py'))
SOURCES = frozenset((
    'crates/iroha_pasta/src/curve/hash_to_curve.rs',
    'crates/iroha_pasta/src/curve/mod.rs',
    'crates/iroha_pasta/src/fft.rs',
    'crates/iroha_pasta/src/field/fp.rs',
    'crates/iroha_pasta/src/field/fq.rs',
    'crates/iroha_pasta/src/field/mod.rs',
    'crates/iroha_pasta/src/params.rs',
    'crates/iroha_pasta/src/poseidon/mod.rs',
    'crates/iroha_pasta/src/poseidon/rp57_fp.bin',
    'crates/iroha_pasta/src/poseidon/rp57_fq.bin',
    'crates/iroha_plonk/src/pcs/ipa/generator_prefix.rs',
    'crates/iroha_plonk/src/pcs/ipa/mod.rs',
    'crates/iroha_plonk/src/transcript/mod.rs',
    'crates/iroha_plonk/src/transcript/pipa_r.rs',
    'crates/iroha_plonk_gadgets/src/poseidon/sponge.rs',
    'crates/iroha_plonk_gadgets/src/pow5_fq/duplex.rs',
    'crates/iroha_plonk_recursion/src/accumulation.rs',
    'crates/iroha_plonk_recursion/src/accumulation_circuit.rs',
    'crates/iroha_plonk_recursion/src/claim.rs',
    'crates/iroha_plonk_recursion/src/fold_tape.rs',
    'crates/iroha_plonk_recursion/src/purpose.rs',
    'crates/iroha_plonk_recursion/src/terminal.rs',
    'crates/iroha_plonk_recursion/src/tests.rs',
    'crates/iroha_plonk_recursion/src/transcript.rs',
    'fixtures/native_prover/kats_v1.json',
    'fixtures/native_prover/reference_verifier/__init__.py',
    'fixtures/native_prover/reference_verifier/curve.py',
    'fixtures/native_prover/reference_verifier/parameters.py',
    'fixtures/native_prover/reference_verifier/transcript.py',
))
INPUT_DOMAIN = int.from_bytes(b'pipa-in1', 'little')
PURPOSES = {'SigmaBatch': (1, b'sigfold1'), 'AInitial': (0, b'a0pfold1'),
            'AContinuation': (0, b'acpfold1'), 'ATerminal': (0, b'atpfold1')}
DOMAIN = int.from_bytes(b'pipa-as1', 'little')
IV = (1 << 64, 0, 0)
K = 16


def require(condition, message):
    """Reject explicitly, including before any subject load under python -O."""
    if not condition:
        raise ValueError(message)


def validate_manifest(manifest):
    """Admit the exact maintained/reference/native source roles, not arbitrary paths."""
    require(type(manifest) is dict and set(manifest) == {'schema', 'files', 'sources'},
            'manifest grammar')
    require(manifest['schema'] == 'kagemusha.fold.transcript-controls.v1', 'manifest schema')
    require(type(manifest['files']) is dict and set(manifest['files']) == FILES,
            'control inventory')
    require(type(manifest['sources']) is dict and set(manifest['sources']) == SOURCES,
            'source inventory')
    for digest in (*manifest['files'].values(), *manifest['sources'].values()):
        require(type(digest) is str and len(digest) == 64 and
                all(c in '0123456789abcdef' for c in digest), 'canonical digest')


def checked_sources():
    """Rehash only the explicit small inputs before loading the unchanged reference."""
    require(sys.flags.optimize == 0, 'unoptimized controls only')
    manifest = json.loads((HERE / 'transcript_source_manifest.json').read_text())
    validate_manifest(manifest)
    for base, rows in ((HERE, manifest['files']), (ROOT, manifest['sources'])):
        for name, digest in rows.items():
            require(hashlib.sha256((base / name).read_bytes()).hexdigest() == digest,
                    'changed source ' + name)
    return manifest


@lru_cache(maxsize=1)
def _load_reference():
    name = '_kagemusha_fold_transcript_reference'
    require(name not in sys.modules, 'reference namespace occupied')
    spec = importlib.util.spec_from_file_location(
        name, REFERENCE / '__init__.py', submodule_search_locations=[str(REFERENCE)])
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return importlib.import_module(name + '.transcript')


@lru_cache(maxsize=1)
def reference():
    """Access unchanged RP57/Curve arithmetic after checking every explicit source role."""
    checked_sources()
    return _load_reference()


def curve(tag):
    """Choose one of the two explicit curve orientations."""
    require(type(tag) is int and tag in (0, 1), 'curve tag')
    return reference().Curve(tag)


def scalar_words(tag, value):
    """Native injective one-word/two-limb scalar encoding, without alias reduction."""
    c = curve(tag)
    require(type(value) is int and 0 <= value < c.scalar, 'canonical scalar')
    if tag == 1:
        return (value,)
    return (value & ((1 << 128) - 1), value >> 128)


def mapped_word(tag, value):
    """Apply the actual full-width map, preserving zero rather than retrying."""
    c = curve(tag)
    require(type(value) is int and 0 <= value < c.base, 'canonical base word')
    return value % c.scalar


def point_words(tag, point):
    """Check canonical finite affine coordinates for a syntactic transcript point."""
    c = curve(tag)
    require(type(point) is tuple and len(point) == 2, 'affine point')
    x, y = point
    require(all(type(v) is int and 0 <= v < c.base for v in point), 'canonical point')
    require((y*y - x*x*x - 5) % c.base == 0, 'point on curve')
    return point


def fixture_inputs(tag, count):
    """Rust prelude KAT inputs at count2; other bounded counts are only syntactic."""
    c = curve(tag)
    require(type(count) is int and 1 <= count <= 16, 'bounded input count')
    short = ((c.base - 1, c.base - 2), 3, (0,) * 13 + (9,) * 3)
    full = ((c.base - 1, 2), 16, tuple(range(2, 18)))
    return (short,) * (count - 1) + (full,)


def context_words(tag, parameter_digest, purpose, count):
    """Exact-binding context only; the closed Terminal/WrapperVesta is excluded.

    These are syntactic schedule words, not parameter or fold admission. The
    bounded control permits at most sixteen sources; native generic roles have
    no corresponding sixteen-source protocol cap.
    """
    curve(tag)
    require(type(parameter_digest) is bytes and len(parameter_digest) == 32,
            'parameter digest extent')
    require(type(count) is int and 1 <= count <= 16, 'bounded input count')
    require(type(purpose) is str and purpose in PURPOSES, 'exact-binding purpose')
    orientation, domain = PURPOSES[purpose]
    require(tag == orientation and (purpose != 'ATerminal' or count == 4), 'purpose shape')
    return (int.from_bytes(parameter_digest[:16], 'little'),
            int.from_bytes(parameter_digest[16:], 'little'),
            int.from_bytes(domain, 'little'), count)


def input_words(tag, inputs):
    """Ordered normalized source syntax; identity G has the native (0,0) frame.

    A syntactic source is not a decided generator claim. D/L/R use point_words
    instead, so the scoped identity frame cannot become a finite proof point.
    """
    c = curve(tag)
    require(type(inputs) is tuple and 1 <= len(inputs) <= 16, 'bounded input list')
    words, full = [], False
    for row in inputs:
        require(type(row) is tuple and len(row) == 3, 'input grammar')
        point, source_k, challenges = row
        require(type(source_k) is int and 1 <= source_k <= K, 'source k')
        require(type(challenges) is tuple and len(challenges) == K, 'challenge count')
        require(all(type(v) is int and 0 <= v < c.scalar for v in challenges),
                'canonical source challenge')
        require(challenges[:K-source_k] == (0,) * (K-source_k) and
                all(v != 0 for v in challenges[K-source_k:]), 'source padding/nonzero')
        full |= source_k == K
        identity = type(point) is tuple and len(point) == 2 and all(
            type(v) is int and v == 0 for v in point)
        words.extend((*(point if identity else point_words(tag, point)), source_k))
        for value in challenges:
            words.extend(scalar_words(tag, value))
    require(full, 'full source required')
    return tuple(words)


def input_digest_words(tag, parameter_digest, purpose, inputs, masks):
    """Separate exact input hash: domain/length/context/sources/all32 masks."""
    source = input_words(tag, inputs)
    context = context_words(tag, parameter_digest, purpose, len(inputs))
    require(type(masks) is tuple and len(masks) == 2*K, 'mask extent')
    encoded = tuple(word for mask in masks for word in scalar_words(tag, mask))
    words = context + source + encoded
    return (INPUT_DOMAIN, len(words)) + words


@lru_cache(maxsize=2)
def k4_parameter_bytes(tag):
    """Rebuild diagnostic k4 bytes from admitted k6 g/W/U, with a fresh IFFT.

    The unchanged reference reconstructs and admits all k6 bytes against the
    existing KAT. Only its coefficient prefix and W/U are reused. The recursive
    sixteen-point transform below never slices the k6 Lagrange basis. This is
    small transcript KAT data; k4 is not added to the reference admission table.
    It does not rerun native hash-to-curve or claim k16 fold parameter support.
    """
    c = curve(tag)
    owner = importlib.import_module(_load_reference().__package__ + '.parameters')
    k6 = owner.Parameters.decode(owner.seed_parameters_k6(c), c, 6)
    root = pow(c.omega(4), -1, c.scalar)

    def transform(values, omega):
        if len(values) == 1:
            return values
        even = transform(values[::2], omega*omega % c.scalar)
        odd = transform(values[1::2], omega*omega % c.scalar)
        half, power, low, high = len(even), 1, [], []
        for i in range(half):
            term = c.multiply(odd[i], power)
            low.append(c.add(even[i], term))
            high.append(c.add(even[i], c.multiply(term, -1)))
            power = power*omega % c.scalar
        return tuple(low + high)

    bases = k6.g[:16]
    scale = pow(16, -1, c.scalar)
    lagrange = tuple(c.multiply(value, scale) for value in transform(bases, root))
    require(all(point[2] != 0 for point in (*bases, *lagrange, k6.w, k6.u)),
            'finite diagnostic parameters')
    return (4).to_bytes(4, 'little') + b''.join(
        c.encode(point) for point in (*bases, *lagrange, k6.w, k6.u))


def commitment_opening(tag, digest, rho):
    """D = g0*lo128(h) + g1*hi128(h) + W*rho for the diagnostic k4 owner."""
    c = curve(tag)
    require(type(digest) is int and 0 <= digest < c.base, 'canonical input digest')
    scalar_words(tag, rho)
    raw = k4_parameter_bytes(tag)
    g0, g1, w = (c.decode(raw[i:i+32]) for i in (4, 36, 4+32*32))
    point = c.sum(((digest & ((1 << 128)-1), g0), (digest >> 128, g1), (rho, w)))
    require(point[2] != 0, 'identity commitment D')
    return point_words(tag, c.affine(point))


def prelude_words(tag, parameter_digest, purpose, count, commitment):
    """Seven challenge words: domain, four context words and finite D(x,y)."""
    return (DOMAIN,) + context_words(tag, parameter_digest, purpose, count) + \
        point_words(tag, commitment)


def committed_case(tag, inputs, purpose, rho, masks):
    """Build a diagnostic Exact prelude from actual RP57 and the k4 opening.

    No L/R equation, native proof, privacy, randomness or Terminal claim follows.
    """
    parameter_digest = hashlib.sha256(k4_parameter_bytes(tag)).digest()
    words = input_digest_words(tag, parameter_digest, purpose, inputs, masks)
    digest_trace = trace_script(tag, (words,))
    check_trace(tag, (words,), digest_trace)
    h = digest_trace['raw'][0]
    d = commitment_opening(tag, h, rho)
    first = prelude_words(tag, parameter_digest, purpose, len(inputs), d)
    return {'parameter_digest': parameter_digest, 'digest_words': words,
            'digest_trace': digest_trace, 'h': h, 'd': d, 'prelude': first}


def padded_blocks(words):
    """Independent native block schedule: pairs, then odd or even tail padding."""
    require(type(words) is tuple and len(words) <= 1024, 'bounded word tuple')
    pairs = tuple((words[i], words[i+1]) for i in range(0, len(words)-1, 2))
    tail = (words[-1], 1) if len(words) % 2 else (1, 0)
    return pairs + (tail,)


def trace_script(tag, buffers):
    """Run unchanged RP57 arithmetic while recording every input/output triple."""
    c, ref = curve(tag), reference()
    require(type(buffers) is tuple and 1 <= len(buffers) <= 32, 'bounded squeezes')
    require(all(type(row) is tuple for row in buffers), 'word tuples')
    require(sum(map(len, buffers)) <= 1024, 'bounded total words')
    require(all(type(v) is int and 0 <= v < c.base for row in buffers for v in row),
            'canonical input word')

    class TraceSponge(ref.Sponge):
        def __init__(self):
            super().__init__(c.base)
            self.edges = []

        def permute(self):
            before = tuple(self.state)
            super().permute()
            self.edges.append((before, tuple(self.state)))

    sponge = TraceSponge()
    raw, endpoints = [], []
    for row in buffers:
        for word in row:
            sponge.absorb(word)
        raw.append(sponge.squeeze())
        endpoints.append(len(sponge.edges))
    return {'raw': tuple(raw), 'mapped': tuple(v % c.scalar for v in raw),
            'edges': tuple(sponge.edges), 'endpoints': tuple(endpoints),
            'state': tuple(sponge.state), 'pending': tuple(sponge.buffer)}


def fold_script(tag, parameter_digest, purpose, count, commitment):
    """All19 syntactic squeezes; finite L/R do not assert the masked IPA equation."""
    c = curve(tag)
    first = prelude_words(tag, parameter_digest, purpose, count, commitment)
    left, right = point_words(tag, (c.base - 1, 2)), point_words(tag, (c.base - 1, c.base - 2))
    return (first, (), ()) + ((*left, *right),) * K


def expected_positions(tag, count):
    """The challenge sponge always has54 calls, separately from the input hash."""
    curve(tag)
    require(type(count) is int and 1 <= count <= 16, 'bounded input count')
    return 4 + 1 + 1 + K*3


def expected_digest_positions(tag, count):
    """Exact-binding input-hash calls including domain, length and final padding."""
    expected_positions(tag, count)
    scalar_width = 2 if tag == 0 else 1
    words = 6 + count*(3 + K*scalar_width) + 2*K*scalar_width
    return words//2 + 1


def role_challenges(tag, mapped):
    """Retain alpha/z/zeta, then nz each round scalar; no extra sponge call."""
    c = curve(tag)
    require(type(mapped) is tuple and len(mapped) == 3+K, 'challenge role extent')
    require(all(type(v) is int and 0 <= v < c.scalar for v in mapped),
            'canonical mapped challenge')
    return mapped[:3] + tuple(reference().nonzero_challenge(v, c.scalar) for v in mapped[3:])


def check_trace(tag, buffers, trace):
    """Compare every observed input with a separately derived carried block state."""
    c = curve(tag)
    prior, cursor = IV, 0
    require(len(buffers) == len(trace['raw']) == len(trace['endpoints']), 'squeeze count')
    for index, row in enumerate(buffers):
        for a, b in padded_blocks(row):
            require(cursor < len(trace['edges']), 'missing primitive')
            before, after = trace['edges'][cursor]
            require(before == (prior[0], (prior[1]+a) % c.base, (prior[2]+b) % c.base),
                    'primitive input schedule')
            require(len(after) == 3 and all(type(v) is int and 0 <= v < c.base for v in after),
                    'canonical output state')
            prior, cursor = after, cursor + 1
        require(trace['endpoints'][index] == cursor and trace['raw'][index] == prior[1],
                'endpoint/projection')
        require(trace['mapped'][index] == prior[1] % c.scalar, 'mapped projection')
    require(cursor == len(trace['edges']) and trace['state'] == prior and trace['pending'] == (),
            'final state/extra primitive')
