"""Independent consumer requirements for resource-observation delivery.

The caller supplies these before reading a report. Neither the report's mode nor
its adjacent module/source record is authority for its own admission.
"""
from __future__ import annotations
import re

ORDINARY = 'ordinary_native_v1'
LINUX = 'linux_owned_root_group_v1'
CONTRACTS = (ORDINARY, LINUX)
PIN_FIELDS = ('module_sha256', 'provenance_sha256', 'source_manifest_sha256', 'native_target')
LINUX_TARGETS = ('x86_64-unknown-linux-gnu', 'x86_64-unknown-linux-musl',
                 'aarch64-unknown-linux-gnu', 'aarch64-unknown-linux-musl')


def sha256(value):
    """Require one nonzero lowercase independent SHA-256 anchor."""
    return type(value) is str and re.fullmatch('[0-9a-f]{64}', value) is not None and value != '0'*64


def validate(expected_contract, expected_delivery):
    """Require a closed independent contract and its complete delivery anchors."""
    if type(expected_contract) is not str or expected_contract not in CONTRACTS:
        raise ValueError('independent observation contract is required')
    if expected_contract == ORDINARY:
        if expected_delivery is not None:
            raise ValueError('ordinary observation cannot carry Linux delivery pins')
        return
    if (type(expected_delivery) is not dict or set(expected_delivery) != set(PIN_FIELDS)
            or not all(sha256(expected_delivery[key]) for key in PIN_FIELDS[:3])
            or type(expected_delivery['native_target']) is not str
            or expected_delivery['native_target'] not in LINUX_TARGETS):
        raise ValueError('complete independent Linux observer delivery pins are required')


def validate_projection(value, expected_delivery=None):
    """Validate the production record and optionally join independently supplied pins."""
    if (type(value) is not dict or set(value) != {'version','kind','cargo_lock_sha256',*PIN_FIELDS}
            or type(value['version']) is not int or value['version'] != 1
            or value['kind'] != 'independently_pinned_native_observer'
            or not sha256(value['cargo_lock_sha256'])):
        raise ValueError('invalid independent observer delivery record')
    validate(LINUX,{key:value[key] for key in PIN_FIELDS})
    if expected_delivery is not None:
        validate(LINUX,expected_delivery)
        if any(value[key] != expected_delivery[key] for key in PIN_FIELDS):
            raise ValueError('observer delivery differs from independent pins')


def add_consumer_arguments(parser,*,required=True):
    """Require the explicit observation mode; Linux additionally needs all four pins."""
    parser.add_argument('--require-observation-contract',required=required,choices=CONTRACTS,
                        help='independent required observation mode, never inferred from a report')
    parser.add_argument('--trusted-observer-sha256')
    parser.add_argument('--trusted-observer-provenance-sha256')
    parser.add_argument('--trusted-source-manifest-sha256')
    parser.add_argument('--observer-native-target')


def arguments(arguments):
    """Decode an explicit CLI expectation before any run evidence is opened."""
    contract=arguments.require_observation_contract
    values=[getattr(arguments,name,None) for name in ('trusted_observer_sha256',
        'trusted_observer_provenance_sha256','trusted_source_manifest_sha256','observer_native_target')]
    delivery=None if all(value is None for value in values) else dict(zip(PIN_FIELDS,values,strict=True))
    validate(contract,delivery)
    return contract,delivery
