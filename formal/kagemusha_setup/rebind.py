"""Private known-log re-keying of pinned public tables.

No import constructs parameters, reads a large artifact or changes repository
authority. Callers supply already retained original bytes and chosen parameters.
Historical inner constants remain unchanged; this is not recursive re-keying.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
import importlib
import importlib.util
import json
from pathlib import Path
import sys

from .custody import (ROOT, REFERENCE, MODULES, require, sha, checked_sources,
                      write_new, save)
from .case import uint, frame
from .bounded import scalar_ifft

def reference(directory, *, parameter_authority=None):
    """Copy the unchanged reference; optionally install one small private authority.

    Every verifier module is copied verbatim. The chosen authority is written
    before any reference module is imported; it never changes release authority.
    """
    return _reference(directory, parameter_authority=parameter_authority)


def _reference(directory, *, parameter_authority=None, writer=write_new, saver=save,
               before_import=lambda: None):
    """Private copy mechanism with an observable authority-before-import boundary."""
    # Refuse oversized authority before any path creation or raw hash.
    if parameter_authority is not None:
        require(isinstance(parameter_authority, (tuple, list)) and len(parameter_authority) == 3,
                'private authority shape')
        tag, k, raw = parameter_authority
        require(type(tag) is int and tag in (0, 1) and type(k) is int and 1 <= k <= 16
                and isinstance(raw, bytes), 'private authority')
        require(k <= 6, 'small reference authority k<=6 only')
        require(len(raw) == 64 * (1 << k) + 68, 'private authority extent')
    directory = Path(directory)
    directory.mkdir(mode=0o700, exist_ok=False)
    package_name = '_generic_logged_'+sha(str(directory.resolve()).encode())[:20]
    require(package_name not in sys.modules, 'private namespace already loaded')
    package = directory/package_name
    package.mkdir()
    changes = []
    for name in MODULES:
        raw = (REFERENCE/name).read_bytes()
        writer(package/name, raw)
        changes.append({'name':name, 'before_sha256':sha(raw), 'after_sha256':sha(raw)})
    kats = json.loads((ROOT/'fixtures/native_prover/kats_v1.json').read_text())
    if parameter_authority is not None:
        tag, k, raw = parameter_authority
        kats['params_ipa'] = {'ep':[], 'eq':[]}
        kats['params_ipa']['ep' if tag == 0 else 'eq'] = [
            {'k':k, 'byte_len':len(raw), 'sha256':sha(raw)}]
    saver(directory/'kats_v1.json', kats)
    saver(directory/'copy-receipt.json', {'source_changes':changes,
         'authority_before_import':True, 'no_transcript_or_equation_changes':True})
    before_import()
    spec = importlib.util.spec_from_file_location(package_name, package/'__init__.py',
                                                  submodule_search_locations=[str(package)])
    module = importlib.util.module_from_spec(spec)
    sys.modules[package_name] = module
    spec.loader.exec_module(module)
    return {name:importlib.import_module(package_name+'.'+name)
            for name in ('codec', 'curve', 'descriptor', 'parameters', 'transcript', 'verify')}


def rebound_descriptor(raw, params_digest, codec):
    """Replace only params_digest, preserving every other canonical field."""
    require(isinstance(params_digest, bytes) and len(params_digest) == 32, 'parameter digest')
    cursor, fields = codec.descriptor_frame(raw, 2), []
    while cursor.position < len(cursor.data):
        fields.append(cursor.read(cursor.length()))
    cursor.finish()
    require(len(fields) == 25 and len(fields[4]) == 32, 'exact V2 descriptor fields')
    fields[4] = params_digest
    payload = b''.join(frame(value) for value in fields)
    schema = hashlib.sha256(b'norito:v1:type-name\0iroha.plonk.pipa.circuit_descriptor.v2').digest()[:16]
    return (b'NRT0\0\0'+schema+b'\0'+uint(len(payload), 8)+
            uint(codec.crc64(payload), 8)+b'\x02'+payload)


@dataclass(frozen=True)
class Case:
    """Private re-keyed historical polynomial relation and checked point logs."""
    directory: Path
    verifier: object
    transcript: object
    descriptor: object
    key: bytes
    raw_params: bytes
    params: object
    base: tuple
    logs: dict
    public_setup: object
    source_descriptor_sha256: str
    public_original: bytes


def make_case(directory, descriptor_raw, key, original, expected, chosen):
    """Re-key one small source relation using exact caller-retained originals.

    Caller-selected hashes are custody inputs, not signatures. This function
    consumes supplied bytes; it never discovers or opens catalog originals.
    """
    checked_sources()
    require(type(chosen.k) is int and 1 <= chosen.k <= 6, 'small rebind k<=6 only')
    require(isinstance(original, bytes) and len(original) <= 1 << 20,
            'small public original extent')
    from .public_setup import PublicSetup
    require(set(expected) == {'descriptor', 'key', 'original'}, 'exact original pin roles')
    for name, raw, limit in [('descriptor', descriptor_raw, 1 << 20),
                             ('key', key, 4 << 20), ('original', original, 1 << 20)]:
        require(isinstance(raw, bytes) and len(raw) <= limit and sha(raw) == expected[name],
                'pinned original '+name)
    directory = Path(directory).resolve()
    directory.mkdir(mode=0o700, parents=True, exist_ok=False)
    for name, raw in [('descriptor.norito', descriptor_raw), ('vk.bin', key), ('public-original.bin', original)]:
        write_new(directory/('historical-'+name), raw)
    old = reference(directory/'historical-reference')
    d = old['descriptor'].Descriptor.decode(descriptor_raw, 2)
    require(d['k'] <= 6, 'small source relation k<=6 only')
    old['verify'].key_points(d, key)
    require(d['transcript'] == 2 and d['instance_mode'] == 1 and d['proof_suffix'] == 1,
            'direct PIPA-R suffix profile')
    public = PublicSetup.decode(original, d, key, expected['original'])
    return _finish_case(directory, descriptor_raw, d, key, original, expected, chosen, old, public)


def _finish_case(directory, descriptor_raw, d, key, original, expected, chosen, old, public, *,
                 writer=write_new, saver=save,
                 checkpoint=lambda: None):
    """One known-log re-key algorithm for the retained small relations."""
    from .public_setup import PublicSetup
    curve, m, n = d.curve, d.curve.scalar, d.n
    require(chosen.curve_tag == d['curve'] and chosen.k == d['k'], 'chosen curve/domain')
    require(isinstance(chosen.raw, bytes) and len(chosen.raw) == 64*n+68,
            'chosen parameter extent')
    require(set(chosen.logs) == {'g', 'lagrange', 'w', 'u'}, 'exact chosen log roles')
    g, lag = chosen.logs['g'], chosen.logs['lagrange']
    w, u = chosen.logs['w'], chosen.logs['u']
    require(type(g) is tuple and type(lag) is tuple and len(g) == len(lag) == n,
            'chosen log vectors')
    require(all(type(v) is int and 0 < v < m for v in (*g, *lag, w, u)), 'finite canonical parameter logs')
    require(lag == scalar_ifft(g, curve, d['k'], checkpoint), 'chosen Lagrange IFFT')
    base = chosen.base
    require(curve.equal(curve.decode(curve.encode(base)), base), 'chosen finite base')
    expected_params = _parameter_bytes(curve,base,d['k'],g,lag,w,u,checkpoint)
    require(isinstance(chosen.raw, bytes) and chosen.raw == expected_params,
            'chosen raw parameters and all private logs')
    new_desc = rebound_descriptor(descriptor_raw, hashlib.sha256(chosen.raw).digest(), old['codec'])
    checkpoint()
    private = reference(directory/'chosen-reference',
                        parameter_authority=(d['curve'], d['k'], chosen.raw))
    params = private['parameters'].Parameters.decode(chosen.raw, curve, d['k'])
    checkpoint()
    new_d = private['descriptor'].Descriptor.decode(new_desc, 2)
    require({k:v for k,v in new_d.values.items() if k != 'params_digest'} ==
            {k:v for k,v in d.values.items() if k != 'params_digest'}, 'unchanged descriptor relation')
    # Native keygen commits finalized evaluation columns with DEFAULT_BLIND=1.
    logs = [sum(value*log for value, log in zip(column, lag)) % m
            for column in (*public.fixed, *public.sigma)]
    logs = [(value+w) % m for value in logs]
    require(all(log != 0 for log in logs), 'native key identity stop')
    key_points = b''.join(curve.encode(curve.multiply(base, log)) for log in logs)
    end = 10+32*len(logs)
    new_key = key[:10]+key_points+key[end:]
    private['verify'].key_points(new_d, new_key)
    # Keep all selector bitmap bytes and all copy/table bytes unchanged.
    table_offset = 44+len(key)
    new_original = b'PIPAPK01'+new_d.digest+uint(len(new_key))+new_key+original[table_offset:]
    checkpoint()
    setup = PublicSetup.decode(new_original, new_d, new_key, sha(new_original))
    checkpoint()
    require(setup.fixed == public.fixed and setup.sigma == public.sigma and
            setup.copy_digest == public.copy_digest, 'unchanged public tables/copy digest')
    for name, raw in [('descriptor.norito', new_desc), ('vk.bin', new_key),
                      ('public-original.bin', new_original), ('parameters.bin', chosen.raw)]:
        writer(directory/('chosen-'+name), raw)
    count = d['num_fixed_columns']
    all_logs = dict(chosen.logs, fixed=tuple(logs[:count]), sigma=tuple(logs[count:]))
    saver(directory/'private-logs.json', {'not_public_protocol_data':True, 'logs':all_logs})
    saver(directory/'rebind-receipt.json', {'historical':expected,
         'chosen':{'descriptor':sha(new_desc), 'key':sha(new_key), 'original':sha(new_original),
                   'parameters':sha(chosen.raw)}, 'default_key_blind':1,
         'same_relation_fields_except_parameter_digest':True, 'same_public_tables':True,
         'same_copy_digest_and_selector_bytes':True,
         'scope':'Historical polynomial relation with chosen outer commitment parameters; no recursively regenerated source catalog or current release admission.'})
    return Case(directory, private['verify'], private['transcript'], new_d, new_key,
                chosen.raw, params, base, all_logs, setup, expected['descriptor'], new_original)


def _parameter_bytes(curve,base,k,g,lag,w,u,checkpoint):
    """Same ordered point reconstruction, checkpointing each bounded point."""
    encoded=bytearray()
    for value in (*g,*lag,w,u):
        encoded.extend(curve.encode(curve.multiply(base,value)))
        checkpoint()
    return uint(k)+bytes(encoded)
