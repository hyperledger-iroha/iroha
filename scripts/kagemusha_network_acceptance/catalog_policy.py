"""Bounded current 22-route catalog receipt policy, with no native execution.

Counts come from the current compiler's authenticated-inventory output. The
18 built-in registry indices and previous 52-route captures confer no authority.
Norito objects remain native Rust's responsibility: this policy binds their
retained bytes to the successful selected producer and its source pins.
"""
from pathlib import Path
import hashlib
import json
import os
import re
import stat
import sys

if sys.flags.optimize:
    raise SystemExit('optimized Python is unsupported')
from paths import ROOT, QUAL
HERE = Path(__file__).resolve().parent
TEST = 'kagemusha_wallet_artifacts_v1::producer_inventory::compiler::full_catalog::complete_wallet_catalog_from_pinned_genesis'
SCOPE = ('Current-source engineering native-genesis 22-route catalog construction, signing and strict source qualification only. '
         'No deployment authority, genuine receipt, native wallet open, A-to-B-to-C exchange, four-validator completion, phone or release qualification.')
OUTPUT_CAPS = {
    'engineering-source-acceptance.norito': 16 << 10,
    'qualified-source-membership.json': 16 << 20,
    'producer-inventory.norito': 16 << 20,
    'engineering-verifier-pack.norito': (16 << 20) + (64 << 10),
    'engineering-installation.norito': 16 << 10,
    'input-pins.norito': 16 << 10,
    'binary.sha256': 32,
    'source.sha256': 32,
    'producer-inventory.sha256': 32,
}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def pin(value):
    require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None and value != '0' * 64, 'nonzero lowercase SHA-256 required')
    return value


def identity(info):
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def bounded(path, cap):
    path = Path(path)
    require(path.is_absolute() and path.resolve(strict=True) == path, 'unaliased absolute original required')
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= cap, 'nonempty bounded regular original required: ' + str(path))
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        data = stream.read(cap + 1)
        after = os.fstat(stream.fileno())
    require(len(data) == before.st_size and identity(before) == identity(opened) == identity(after) == identity(path.lstat()), 'original changed while read')
    return data


def sha(path):
    path = Path(path)
    require(path.is_absolute() and path.resolve(strict=True) == path, 'unaliased hash original required')
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode), 'regular hash original required')
    digest = hashlib.sha256()
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        for block in iter(lambda: stream.read(1 << 20), b''):
            digest.update(block)
        after = os.fstat(stream.fileno())
    require(identity(before) == identity(opened) == identity(after) == identity(path.lstat()), 'hash input changed')
    return digest.hexdigest()


def doc(path, expected=None):
    data = bounded(path, 32 << 20)
    if expected is not None:
        require(hashlib.sha256(data).hexdigest() == pin(expected), 'selected document hash differs')
    return strict_json(data)


def save(path, value):
    with Path(path).open('x') as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())
    return {'path': str(path), 'sha256': sha(path)}


def inherited_environment(env):
    rejected = sorted(k for k in env if k.startswith(('KAGEMUSHA_', 'IROHA_TEST_', 'TEST_NETWORK_', 'RUST_TEST_', 'LD_', 'DYLD_')) or k == 'PYTHONOPTIMIZE')
    require(not rejected, 'inherited subject/reuse/loader settings refused: ' + ', '.join(rejected))
    return dict(env)


def compiled_inventory(stdout, stderr, code):
    require(type(code) is int and code == 0 and not stderr.strip(), 'compiled inventory failed')
    lines = [line for line in stdout.splitlines() if line.strip()]
    require(lines == [TEST + ': test', '1 test, 0 benchmarks'], 'exact one compiled ignored test required')


def test_result(stdout, code):
    require(type(code) is int and code == 0, 'native test exit was not numeric zero')
    # libtest may announce its one serial test as slow after it prints the prefix.
    slow = f'test {TEST} has been running for over 60 seconds'
    normalized = stdout.replace(slow + '\n', '')
    statuses = re.findall(r'^test ([^\n]+?) \.\.\. (ok|FAILED|ignored)\s*$', normalized, re.M)
    summaries = re.findall(r'^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in [0-9.]+s\s*$', normalized, re.M)
    require(statuses == [(TEST, 'ok')], 'exact selected named test did not pass')
    require(len(summaries) == 1 and summaries[0][:5] == ('ok', '1', '0', '0', '0'), 'exact one-pass summary required')
    require(len(re.findall(r'^running 1 test$', normalized, re.M)) == 1, 'exact runtime test inventory missing')
    return {'named': TEST, 'passed': 1, 'failed': 0, 'ignored': 0, 'filtered': int(summaries[0][5])}


# Source contracts: producer_inventory.rs::validate, schedule/compiled.rs,
# compiler/full_catalog.rs and full_catalog/{acceptance,acquisition_tests,transport}.
ROUTES = 22
SIGMA_IMPORTS = 11
ACQUISITION_CASES = 14
ORIGINAL_COUNT_CAP = 4096
ORIGINAL_BYTES_CAP = 1 << 30
STORED_BYTES_CAP = 128 << 30
MEMBERSHIP_SCHEMA = 'iroha.kagemusha.engineering-qualified-source-membership.v1'
MEMBERSHIP_SCOPE = ('Exact signed inventory membership after complete source qualification; '
                    'no phone storage, memory, performance or deployment authority')
POSITIVE = r'([1-9][0-9]{0,19})'


def unique_object(rows):
    result = {}
    for name, value in rows:
        require(name not in result, 'duplicate JSON field: ' + name)
        result[name] = value
    return result


def invalid_constant(value):
    raise ValueError('nonfinite JSON number: ' + value)


def strict_json(raw):
    return json.loads(raw, object_pairs_hook=unique_object,
                      parse_constant=invalid_constant)


def natural(value, maximum, label):
    require(type(value) is int and 0 < value <= maximum,
            'positive bounded plain integer required: ' + label)
    return value


def marker(stderr, name, pattern):
    require(type(stderr) is str, 'retained stderr text required')
    lines = [line for line in stderr.splitlines() if line.startswith(name)]
    require(len(lines) == 1, 'exact one ' + name + ' marker required')
    matched = re.fullmatch(pattern, lines[0])
    require(matched is not None, 'current 22-route ' + name + ' marker differs')
    return matched.groups(), lines[0]


def accepted_marker(stderr):
    pattern = (r'WALLET_SOURCE_ACCEPTED engineering=true deployment_authority=false complete_routes=22 '
               r'strict_original_reads=' + POSITIVE + r' active_sigma_imports=11 active_q_imports=' + POSITIVE +
               r' cancelled_import_opens=0 unavailable_retry=true active_seal_acquisition_cases=14 '
               r'receive_alias_cases=' + POSITIVE + r' signature_reuse_cases=' + POSITIVE +
               r' scheme=([0-9a-f]{64}) manifest=([0-9a-f]{64}) genuine_receipt=false native_wallet_open=false')
    values, raw = marker(stderr, 'WALLET_SOURCE_ACCEPTED', pattern)
    reads, q, aliases, signatures = [natural(int(v), (1 << 64) - 1, 'acceptance count') for v in values[:4]]
    scheme, manifest = values[4:]
    return {'routes': ROUTES, 'strict_original_reads': reads, 'sigma_imports': SIGMA_IMPORTS,
            'q_imports': q, 'active_seal_acquisition_cases': ACQUISITION_CASES,
            'receive_alias_cases': aliases, 'signature_reuse_cases': signatures,
            'scheme': pin(scheme), 'manifest': pin(manifest), 'raw': raw}


def catalog_marker(stderr):
    pattern = (r'WALLET_SOURCE_CATALOG routes=22 programs=' + POSITIVE +
               r' terminal_keys=' + POSITIVE + r' originals=' + POSITIVE +
               r' stored_bytes=' + POSITIVE +
               r' signed=false qualified_wallet=false actual_receipt_proof=false')
    values, raw = marker(stderr, 'WALLET_SOURCE_CATALOG', pattern)
    programs, terminals, originals, stored = map(int, values)
    natural(programs, ROUTES, 'generated programs')
    natural(terminals, min(programs, 32), 'terminal keys')
    natural(originals, ORIGINAL_COUNT_CAP, 'inventory originals')
    natural(stored, STORED_BYTES_CAP, 'stored bytes')
    return {'routes': ROUTES, 'programs': programs, 'terminal_keys': terminals,
            'originals': originals, 'stored_bytes': stored, 'raw': raw}


def membership(raw):
    value = strict_json(raw)
    require(type(value) is dict and set(value) ==
            {'schema', 'scope', 'wallet_originals', 'wallet_unique_bytes'},
            'exact qualified membership shape required')
    require(value['schema'] == MEMBERSHIP_SCHEMA and value['scope'] == MEMBERSHIP_SCOPE,
            'current engineering-only wallet membership required')
    rows = value['wallet_originals']
    require(type(rows) is list and 0 < len(rows) <= 3 * ORIGINAL_COUNT_CAP,
            'bounded nonempty wallet originals required')
    previous = ''
    total = 0
    for row in rows:
        require(type(row) is dict and set(row) == {'sha256', 'bytes'},
                'exact wallet original identity required')
        digest = pin(row['sha256'])
        require(digest > previous, 'wallet originals must be sorted and unique')
        previous = digest
        total += natural(row['bytes'], ORIGINAL_BYTES_CAP, 'wallet original bytes')
    require(total == natural(value['wallet_unique_bytes'], STORED_BYTES_CAP, 'wallet unique bytes'),
            'wallet unique byte sum differs')
    return value


def completion_details(catalog, declared, stderr):
    generated = catalog_marker(stderr)
    accepted = accepted_marker(stderr)
    require(stderr.index(generated['raw']) < stderr.index(accepted['raw']),
            'catalog generation must precede acceptance')
    # q_imports visits each concrete program once, each with 1..3 fixed Q roles.
    # No registry table count is a substitute for the measured generated count.
    require(generated['programs'] <= accepted['q_imports'] <= 3 * generated['programs'],
            'active Q imports disagree with generated program bounds')
    raw = bounded(catalog / 'qualified-source-membership.json', OUTPUT_CAPS['qualified-source-membership.json'])
    observed = declared['qualified-source-membership.json']
    require(len(raw) == observed['bytes'] and hashlib.sha256(raw).hexdigest() == observed['sha256'],
            'qualified membership changed between reads')
    members = membership(raw)
    require(len(members['wallet_originals']) <= 3 * generated['originals'],
            'wallet membership exceeds inventory original identities')
    # A reused store can also hold unselected DATA, so equality would be false.
    require(members['wallet_unique_bytes'] <= generated['stored_bytes'],
            'qualified wallet bytes exceed generated store bytes')
    return {'generated': generated, 'accepted': accepted,
            'wallet_originals': len(members['wallet_originals']),
            'wallet_unique_bytes': members['wallet_unique_bytes']}


def completed_outputs(catalog, binary_sha, source_sha):
    require(catalog.resolve(strict=True) == catalog and stat.S_ISDIR(catalog.lstat().st_mode), 'ordinary catalog directory required')
    before = catalog.lstat()
    result = {}
    for name, cap in OUTPUT_CAPS.items():
        data = bounded(catalog / name, cap)
        result[name] = {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
    for name, expected in [('binary.sha256', binary_sha), ('source.sha256', source_sha), ('producer-inventory.sha256', result['producer-inventory.norito']['sha256'])]:
        require(bounded(catalog / name, 32) == bytes.fromhex(pin(expected)), 'catalog raw original digest differs: ' + name)
    require(identity(catalog.lstat()) == identity(before), 'catalog namespace changed during terminal inspection')
    return result
