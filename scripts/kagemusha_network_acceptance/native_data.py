"""Native campaign DATA parsing; Rust alone admits catalog and monetary authority."""
from pathlib import Path
import hashlib
import json
import os
import re
import stat
import custody as bounded
from paths import ROOT, QUAL
CAP = 64 << 20
JSON_LIMIT = CAP
EVIDENCE_LIMIT = 128 << 20
# Mirrors the current native model's complete KAGEMUSHA message bound.
MESSAGE_LIMIT = 10_034
SOURCE_PINS = (
 'KAGEMUSHA_SIGNED_GENESIS_FIXTURE_SHA256',
 'KAGEMUSHA_WALLET_CATALOG_PRODUCER_SHA256', 'KAGEMUSHA_WALLET_CATALOG_SOURCE_SHA256',
 'KAGEMUSHA_WALLET_CATALOG_INVENTORY_SHA256', 'KAGEMUSHA_WALLET_CATALOG_PACK_SHA256',
 'KAGEMUSHA_WALLET_SCHEME_ID', 'KAGEMUSHA_WALLET_MANIFEST_DIGEST',
)
SETUP_NAMES = {'account.norito', 'account-b.norito', 'account-c.norito', 'reserve.norito',
 'asset.norito', 'signed-genesis.wire', 'genesis-manifest.json', 'registration-proof.norito', 'capture.json'}
TARGET_NAMES = {'account.norito', 'asset.norito', 'credential.norito', 'certificates.norito',
 'bootstrap.norito', 'activation.norito', 'issue-load.norito'}
ARCHIVE_NAMES = {'a-credited-receive.norito', 'b-credited-status.norito',
 'a-archive-receive.norito', 'b-archive-status.norito'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def pin(value):
    require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value)
            and value != '0' * 64, 'nonzero lowercase SHA256 required')
    return value


def local(path):
    path = Path(path)
    require(path.is_absolute() and path.is_relative_to(QUAL) and '..' not in path.parts,
            'absolute original-checkout qualification path required')
    require(path.resolve() == path, 'symlink or alias in selected path')
    return path


def observe(path, cap=CAP):
    path = local(path)
    item = bounded.inspect(path, cap)
    return {'path': str(path), 'sha256': item['sha256'], 'bytes': item['bytes']}


def log_pin(path):
    path = local(path)
    if path.lstat().st_size == 0:
        digest = hashlib.sha256(b'').hexdigest()
        exact(path, digest, allow_empty=True)
        return digest
    return observe(path)['sha256']


def retained_text(path, expected=None):
    digest = log_pin(path)
    if expected is not None:
        require(digest == pin(expected), 'selected log digest differs')
    fd = os.open(local(path), os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0))
    try:
        with os.fdopen(fd, 'rb', closefd=False) as reader:
            data = reader.read(CAP + 1)
    finally:
        os.close(fd)
    require(len(data) <= CAP and hashlib.sha256(data).hexdigest() == digest
            and log_pin(path) == digest, 'retained text changed or exceeded bound')
    return data.decode()


def selected(ref, cap=CAP):
    require(type(ref) is dict and set(ref) == {'path', 'sha256'}, 'exact path/pin reference required')
    path = local(ref['path'])
    require(observe(path, cap)['sha256'] == pin(ref['sha256']), 'selected original changed: ' + str(path))
    return path


def document(ref, cap=CAP):
    return selected_json(selected(ref, cap), ref['sha256'], cap)


def manifest_rows(root, rows, expected, *, cap=CAP):
    require(type(cap) is int and 0 < cap <= CAP, 'finite original bound required')
    require(type(rows) is list and len(rows) == len(expected), 'original count differs')
    seen, result = set(), {}
    for row in rows:
        require(type(row) is dict and set(row) == {'name', 'bytes', 'sha256'}, 'original row shape')
        name = row['name']
        require(type(name) is str and Path(name).name == name and name in expected
                and name not in seen, 'original name is duplicate, foreign or escaping')
        seen.add(name)
        require(type(row['bytes']) is int and 0 < row['bytes'] <= cap, 'original length bound')
        got = observe(root / name, cap)
        require(got['sha256'] == pin(row['sha256']) and got['bytes'] == row['bytes'],
                'original hash/extent differs')
        result[name] = got
    require(seen == set(expected), 'missing original')
    return result


def summaries_ok(code, text):
    return type(code) is int and code == 0 and re.findall(
        r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;', text
    ) == [('ok', '1', '0', '0')]
def exact(path, expected, limit=EVIDENCE_LIMIT, allow_empty=False):
    before = path.lstat()
    if allow_empty and stat.S_ISREG(before.st_mode) and before.st_size == 0:
        fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0))
        try:
            opened = os.fstat(fd)
            require(bounded.identity(before) == bounded.identity(opened)
                    == bounded.identity(path.lstat()) and os.read(fd, 1) == b''
                    and bounded.identity(opened) == bounded.identity(os.fstat(fd))
                    == bounded.identity(path.lstat()), 'empty evidence changed during read')
        finally:
            os.close(fd)
        observed = hashlib.sha256(b'').hexdigest()
    else:
        observed = bounded.inspect(path, limit)['sha256']
    require(observed == pin(expected), 'retained evidence digest differs: ' + str(path))


def selected_json(path, expected=None, cap=None):
    cap = JSON_LIMIT if cap is None else cap
    require(type(cap) is int and 0 < cap <= JSON_LIMIT, 'finite JSON bound required')
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= cap,
            'JSON original is not bounded regular data')
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0))
    try:
        opened = os.fstat(fd)
        require(bounded.identity(before) == bounded.identity(opened), 'JSON changed during open')
        chunks = []
        total = 0
        while True:
            chunk = os.read(fd, min(65536, cap + 1 - total))
            if not chunk:
                break
            total += len(chunk)
            require(total <= cap, 'JSON grew beyond bound')
            chunks.append(chunk)
        require(total == opened.st_size and bounded.identity(opened) == bounded.identity(os.fstat(fd))
                == bounded.identity(path.lstat()), 'JSON changed during read')
    finally:
        os.close(fd)
    data = b''.join(chunks)
    if expected is not None:
        require(hashlib.sha256(data).hexdigest() == pin(expected), 'selected JSON digest differs')
    def unique(items):
        result = {}
        for key, value in items:
            require(key not in result, 'duplicate JSON field')
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique)
def ref(record):
    return {key: record[key] for key in ('path', 'sha256')}


def output_records(name, output, env, *, expected_heights=None):
    def doc(filename, schema=None):
        item = observe(output / filename)
        value = document(ref(item))
        if schema:
            require(value.get('schema') == schema, 'wrong stage output schema')
        return item, value
    if name == 'target':
        item, value = doc('public-target/target.json', 'iroha.kagemusha.native-load-target.v1')
        require(value['source_pins'] == {key: env[key] for key in SOURCE_PINS}
                and value['executed_ledger_setup_sha256'] == env['KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256'], 'target source mismatch')
        return {'target': item, **manifest_rows(output / 'public-target', value['files'], TARGET_NAMES)}
    if name == 'activation-retention':
        item, value = doc('component-scope.json', 'iroha.kagemusha.native-software-activation-retention.v1')
        retained, receipt = doc('activation-retained.json', 'iroha.kagemusha.wallet-activation-retained.v1')
        prepared = observe(output / 'activation-prepared.json', 16384)
        wire = observe(output / 'signed-activation.wire', 16 << 20)
        require(value['source_pins'] == {key: env[key] for key in SOURCE_PINS}
                and value['prepared_sha256'] == prepared['sha256'] == env['KAGEMUSHA_NATIVE_ACTIVATION_PREPARED_SHA256']
                and value['target_sha256'] == env['KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256']
                and value['retention_sha256'] == retained['sha256']
                and type(value['new_payment_signatures']) is int and value['new_payment_signatures'] == 0
                and value['physical_device_qualified'] is False, 'native activation retention scope differs')
        require(set(receipt) == {'schema', 'action', 'signedWireSHA256', 'signedWireBytes', 'nativeResult',
                                'entrypointHex', 'nativeReturnedTicks', 'invocation', 'sequence'}
                and receipt['action'] == 'retain-activation' and receipt['nativeResult'] == 'awaiting'
                and pin(receipt['entrypointHex'])
                and receipt['signedWireSHA256'] == wire['sha256'] and receipt['signedWireBytes'] == str(wire['bytes'])
                and receipt['invocation'] == env['KAGEMUSHA_NATIVE_ACTIVATION_INVOCATION']
                and receipt['sequence'] == '0' and type(receipt['nativeReturnedTicks']) is str
                and re.fullmatch('[1-9][0-9]*', receipt['nativeReturnedTicks'])
                and int(receipt['nativeReturnedTicks']) < (1 << 64), 'native activation retention result differs')
        return {'component-scope': item, 'retention': retained, 'prepared': prepared, 'wire': wire}
    if name in {'funding', 'load'}:
        require(type(expected_heights) is dict, 'explicit current funded height receipt required')
        registered = expected_heights.get('registration_height')
        activation = expected_heights.get('activation_height')
        height = expected_heights.get('certified_height')
        require(all(type(number) is int for number in (registered, activation, height))
                and 3 <= registered and activation == registered + 1 and height == activation + 1
                and height <= 64, 'exact bounded funding height sequence required')
    if name == 'funding':
        item, value = doc('capture.json')
        require(value['native_target_sha256'] == env['KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256'], 'funding target differs')
        names = {f'block-{i}.wire' for i in range(1, height + 1)} | {f'native-proof-{i}.norito' for i in range(1, height + 1)} | {'receipt.norito'}
        return {'capture': item, **manifest_rows(output, value['originals'], names)}
    if name == 'load':
        # The Rust exporter verifies native BLS certificates and exact event inclusion.
        item, value = doc('generated.json', 'iroha.kagemusha.generated-native-load-finality.v1')
        receipt, finality = observe(output / 'receipt.norito', 512), observe(output / 'load-finality.norito', 262144)
        require(receipt['sha256'] == env['KAGEMUSHA_EXECUTED_LOAD_RECEIPT_SHA256'], 'Load receipt was replaced')
        require(value.get('native_bls_finality') is True and type(value.get('receipt_height')) is int and value['receipt_height'] == height
                and type(value.get('registration_height')) is int and value['registration_height'] == registered
                and type(value.get('activation_height')) is int and value['activation_height'] == activation and value.get('existing_only_recovery') is True
                and value['receipt_sha256'] == receipt['sha256'] and value['finality_sha256'] == finality['sha256']
                and value['receipt_bytes'] == receipt['bytes'] and value['finality_bytes'] == finality['bytes']
                and value['capture_sha256'] == env['KAGEMUSHA_EXECUTED_LOAD_CAPTURE_SHA256']
                and value['target_sha256'] == env['KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256']
                and value['setup_sha256'] == env['KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256']
                and value['source_pins'] == {key: env[key] for key in SOURCE_PINS}, 'Load completion binding differs')
        return {'generated': item, 'receipt': receipt, 'finality': finality}
    if name == 'exchange':
        item, value = doc('result.json', 'iroha.kagemusha.native-abc-result.v1')
        claim = observe(output / 'c-unload-claim.norito', 16384)
        require(value['ledger_settlement_executed'] is False and value['physical_device_qualified'] is False
                and value['c_unload_claim_sha256'] == claim['sha256']
                and value['target_sha256'] == env['KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256']
                and value['executed_ledger_setup_sha256'] == env['KAGEMUSHA_NATIVE_LEDGER_SETUP_SHA256']
                and value['source_pins'] == {key: env[key] for key in SOURCE_PINS}, 'exchange scope/binding differs')
        # These are observations of the current admitted Rust test, not Python
        # verification of Archive proofs. Preserve all exact originals for its
        # existing artifact custody and final recheck.
        require(all(value.get(key) is True for key in (
            'archive_receive_folded', 'archive_status_folded',
            'archive_exact_replay_after_restart')), 'both Archive folds and restart replay required')
        result = {'exchange': item, 'claim': claim}
        for filename in ('a-b-payment.norito', 'b-c-payment.norito', 'b-receive.norito', 'c-receive.norito'):
            result[filename] = observe(output / filename, MESSAGE_LIMIT)
        result.update(manifest_rows(output, value.get('archive_originals'), ARCHIVE_NAMES, cap=MESSAGE_LIMIT))
        for filename, original in manifest_rows(output / 'wallet-c', value['c_open_originals'],
                                                {'credential.norito', 'certificates.norito', 'account.norito', 'asset.norito'}).items():
            result['c-' + filename] = original
        return result
    raise ValueError('unknown native stage: ' + name)
