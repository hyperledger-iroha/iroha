"""Join the live network genesis to the current 22-route catalog receipt policy."""
from pathlib import Path
import hashlib
import os
import stat
import types
import sys

from paths import ROOT

REUSE_ROOT = Path(__file__).resolve().parent

if sys.flags.optimize:
    raise SystemExit('optimized Python is unsupported')

POLICY = Path(__file__).resolve().with_name('catalog_policy.py')
MANIFEST = POLICY.with_name('manifest.json')


def load_policy(expected_policy):
    """Compile the exact reviewed local source, never a cached bytecode file."""
    path = POLICY
    if path.resolve(strict=True) != path:
        raise ValueError('selected catalog policy alias')
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or not 0 < before.st_size <= 1 << 20:
        raise ValueError('selected catalog policy extent')
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        opened = os.fstat(stream.fileno())
        raw = stream.read((1 << 20) + 1)
        after = os.fstat(stream.fileno())
    identity = lambda info: (info.st_dev, info.st_ino, info.st_mode, info.st_size,
                             info.st_mtime_ns, info.st_ctime_ns)
    if (len(raw) != before.st_size or
            not identity(before) == identity(opened) == identity(after) == identity(path.lstat()) or
            hashlib.sha256(raw).hexdigest() != expected_policy):
        raise ValueError('selected catalog policy original differs')
    module = types.ModuleType('native_network_current_catalog_policy')
    module.__file__ = str(path)
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module


CATALOG_POLICY = load_policy('8d40fe8d84c79db1ae0c438f2c13aaa3e441810ff3d435c73dc31f76b12c2946')


def load_reuse():
    """Compile pinned local DATA helpers, without historical code or cached bytecode."""
    pins = {
        'raw_reuse_policy': '251fff1f8339e3b85ec371dff833437f1ea8f2c2640a6c8c34746099f7372cfe',
        'raw_reuse': '1ce2e912be5f8445395c89cef6a821d230c626c3b30d0e12d5de6d169eb4c949',
    }

    def source(path, expected, name):
        if path.resolve(strict=True) != path:
            raise ValueError('selected raw DATA intake helper alias: ' + name)
        before = path.lstat()
        if not stat.S_ISREG(before.st_mode) or not 0 < before.st_size <= 1 << 20:
            raise ValueError('selected raw DATA intake helper extent: ' + name)
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
        with os.fdopen(fd, 'rb') as stream:
            opened = os.fstat(stream.fileno())
            raw = stream.read((1 << 20) + 1)
            after = os.fstat(stream.fileno())
        identity = lambda info: (info.st_dev, info.st_ino, info.st_mode, info.st_size,
                                 info.st_mtime_ns, info.st_ctime_ns)
        if (len(raw) != before.st_size or
                not identity(before) == identity(opened) == identity(after) == identity(path.lstat()) or
                hashlib.sha256(raw).hexdigest() != expected):
            raise ValueError('selected raw DATA intake helper changed: ' + name)
        return raw

    originals = {}
    for name, expected in pins.items():
        path = REUSE_ROOT / (name + '.py')
        originals[name] = source(path, expected, name)
        loaded = sys.modules.get(name)
        if loaded is not None:
            loaded_path = getattr(loaded, '__file__', None)
            if loaded_path is None or Path(loaded_path).resolve(strict=True) != path:
                raise ValueError('foreign preloaded raw DATA intake helper: ' + name)
    loaded_modules = {}
    for name, raw in originals.items():
        path = REUSE_ROOT / (name + '.py')
        module = types.ModuleType(name)
        module.__file__ = str(path)
        sys.modules[name] = module
        exec(compile(raw, str(path), 'exec'), module.__dict__)
        source(path, pins[name], name)
        loaded_modules[name] = module
    return loaded_modules['raw_reuse']


REUSE = load_reuse()


def setup_fixture(value, originals, data):
    data.require(value['schema'] == 'iroha.kagemusha.executed-ledger-setup.v1'
                 and value['registration_height'] == 1 and value['certified_height'] == 2
                 and value['initial_supply_atomic_units'] == '1000', 'exact live H1/H2 setup required')
    capture = data.document(data.ref(originals['capture.json']), 1 << 20)
    data.require(capture['version'] == 1 and capture['chain_id'] == value['chain_id'],
                 'live capture chain differs')
    encoded = capture['signed_genesis_wire_hex']
    data.require(type(encoded) is str and encoded and len(encoded) % 2 == 0,
                 'bounded encoded signed genesis required')
    wire = bytes.fromhex(encoded)
    data.require(wire.hex() == encoded and len(wire) == originals['signed-genesis.wire']['bytes']
                 and hashlib.sha256(wire).hexdigest() == originals['signed-genesis.wire']['sha256'],
                 'live signed genesis original differs')
    # Native Rust authenticates genesis, PoPs, scheme network and source recipes.
    return originals['capture.json']


def completed(catalog, compiler, fixture, data, policy):
    declared = policy.completed_outputs(catalog, compiler['sha256'], compiler['source_sha256'])
    details = policy.completion_details(
        catalog, declared, data.retained_text(catalog.parent / 'run.stderr'))
    accepted = details['accepted']
    outputs = {name: data.observe(catalog / name, policy.OUTPUT_CAPS[name]) for name in declared}
    data.require(all(outputs[name]['sha256'] == row['sha256'] and outputs[name]['bytes'] == row['bytes']
                     for name, row in declared.items()), 'catalog output changed between reads')
    outputs['source-policy.norito'] = data.observe(catalog / 'source-policy.norito', 4096)
    pins = {
        'KAGEMUSHA_SIGNED_GENESIS_FIXTURE_SHA256': fixture['sha256'],
        'KAGEMUSHA_WALLET_CATALOG_PRODUCER_SHA256': compiler['sha256'],
        'KAGEMUSHA_WALLET_CATALOG_SOURCE_SHA256': compiler['source_sha256'],
        'KAGEMUSHA_WALLET_CATALOG_INVENTORY_SHA256': outputs['producer-inventory.norito']['sha256'],
        'KAGEMUSHA_WALLET_CATALOG_PACK_SHA256': outputs['engineering-verifier-pack.norito']['sha256'],
        'KAGEMUSHA_WALLET_SCHEME_ID': accepted['scheme'],
        'KAGEMUSHA_WALLET_MANIFEST_DIGEST': accepted['manifest'],
    }
    data.require(set(pins) == set(data.SOURCE_PINS), 'exact seven native source pins')
    return outputs, pins
