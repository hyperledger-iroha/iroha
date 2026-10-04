"""Native scoped-original OpenAPI contracts, media limits and exact JSON shapes.

These schema tests use public synthetic shape data. They provide no account,
signature, World-root, finality, proof, Native startup or release qualification.
"""

import copy
import json
import re
from pathlib import Path

import pytest
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[2]
MIRRORS = (
    ROOT / 'artifacts/openapi/torii.json',
    ROOT / 'crates/iroha_torii/assets/openapi/torii.json',
    ROOT / 'artifacts/openapi/versions/current/torii.json',
)
READS = (
    ('/v1/ledger/resource-names/{challenge}', 'get', 'ledger.resource_names_state', None, 128 * 1024 * 1024, 'NativeResourceNamesStateV1'),
    ('/v1/ledger/authority-originals', 'post', 'ledger.authority_originals', 64 * 1024, 128 * 1024 * 1024, 'NativeAuthorityOriginalsV1'),
    ('/v1/kagemusha/ordinary/current-wallet', 'post', 'kagemusha.ordinary_wallet_current', 8 * 1024, 64 * 1024 * 1024, 'OrdinaryWalletCurrentOriginalV1'),
    ('/v1/kagemusha/ordinary/mint-issuer-purpose', 'post', 'kagemusha.ordinary_mint_issuer_purpose', 96 * 1024, 128 * 1024 * 1024, 'OrdinaryMintIssuerPurposeOriginalV1'),
    ('/v1/kagemusha/ordinary/top-up/finality', 'post', 'kagemusha.ordinary_mint_finalized', 16 * 1024, 38_273_024, None),
    ('/v1/kagemusha/ordinary/top-up/credit', 'post', 'kagemusha.ordinary_mint_credit', 16 * 1024, 38_285_056, None),
)
SECURITY = [
    {'IrohaCanonicalAccount': [], 'IrohaCanonicalNonce': [], 'IrohaCanonicalSignature': [], 'IrohaCanonicalTimestampMs': []},
    {'IrohaCanonicalWitness': []},
]


def reject_duplicates(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate member ' + key)
        result[key] = value
    return result


@pytest.fixture(scope='module')
def document():
    return json.loads(MIRRORS[0].read_bytes(), object_pairs_hook=reject_duplicates)


def validator(document, name):
    return Draft202012Validator({'$ref': '#/components/schemas/' + name, 'components': document['components']})


@pytest.fixture(scope='module')
def shape_data():
    authority = json.loads((ROOT / 'scripts/tests/fixtures/kagemusha_authority_state_v1.json').read_bytes())
    public = json.loads((ROOT / 'fixtures/kagemusha/participant_enrollment_http_v1.json').read_bytes())
    return {
        'attestation': authority['attestation'], 'world_snapshot': authority['world_snapshot'],
        'network_id': authority['attestation']['body']['network_id'],
        'asset_definition': authority['asset_definition'],
        'signatory': public['signatory_i105'], 'wallet': public['wallet_i105'],
    }


@pytest.mark.parametrize('path,method,stable_id,request_bound,response_bound,json_name', READS)
def test_six_mandatory_reads_have_the_actual_catalog_and_signed_transport(document, path, method, stable_id, request_bound, response_bound, json_name):
    operation = document['paths'][path][method]
    catalog = (ROOT / 'crates/iroha_torii_shared/src/route_catalog.rs').read_text()
    block = catalog.split('"' + stable_id + '"', 1)[1].split('.with_cors_options(true)', 1)[0]
    assert '"' + path + '"' in block
    assert 'RouteEffect::ReadOnly' in block
    assert 'AdmissionPolicy::AuthenticatedAccount' in block
    assert 'AuthenticationPolicy::CanonicalAccountSignature' in block
    assert 'RouteProjections::OPENAPI_AND_SDK' in block
    assert operation['security'] == SECURITY
    assert operation['x-iroha-route-auth'] == {'admission': 'authenticated_account', 'authentication': 'canonical_account_signature', 'schemaVersion': 1, 'stableRouteId': stable_id}
    assert operation['x-iroha-tool-effect'] == 'read'
    assert operation['x-iroha-max-response-bytes'] == response_bound
    content = operation['responses']['200']['content']
    assert set(content) == ({'application/x-norito'} if json_name is None else {'application/json', 'application/x-norito'})
    assert content['application/x-norito']['schema']['maxLength'] == response_bound
    assert content['application/x-norito']['schema']['minLength'] == 1
    assert content['application/x-norito']['schema']['format'] == 'binary'
    assert content['application/x-norito']['schema']['x-iroha-norito-schema'] == operation['x-iroha-norito-response-type']
    if json_name is not None:
        assert content['application/json']['schema'] == {'$ref': '#/components/schemas/' + json_name}
    if request_bound is None:
        assert 'requestBody' not in operation
    else:
        assert operation['x-iroha-max-request-bytes'] == request_bound
        assert operation['requestBody']['required'] is True
        request = operation['requestBody']['content']
        assert set(request) == {'application/x-norito'}
        assert request['application/x-norito']['schema']['maxLength'] == request_bound
        assert request['application/x-norito']['schema']['x-iroha-norito-schema'] == operation['x-iroha-norito-request-type']


def test_authored_mirrors_and_all_new_schema_references_are_valid(document):
    assert all(path.read_bytes() == MIRRORS[0].read_bytes() for path in MIRRORS)
    schemas = document['components']['schemas']
    for name, value in schemas.items():
        if name.startswith(('Native', 'OrdinaryWalletCurrent', 'OrdinaryMintIssuer')):
            Draft202012Validator.check_schema(value)
    def references(value):
        if isinstance(value, dict):
            if '$ref' in value:
                assert value['$ref'].startswith('#/components/schemas/')
                assert value['$ref'].rsplit('/', 1)[1] in schemas
            for nested in value.values():
                references(nested)
        elif isinstance(value, list):
            for nested in value:
                references(nested)
    for path, method, *_ in READS:
        references(document['paths'][path][method])
    for name, value in schemas.items():
        if name.startswith(('Native', 'OrdinaryWalletCurrent', 'OrdinaryMintIssuer')):
            references(value)


@pytest.mark.parametrize('path', ['/v1/kagemusha/ordinary/top-up/finality', '/v1/kagemusha/ordinary/top-up/credit'])
def test_finalized_reads_share_the_actual_binary_selector_and_empty_pending_response(document, path):
    operation = document['paths'][path]['post']
    source = (ROOT / 'crates/iroha_torii_shared/src/ordinary_mint_finalized.rs').read_text()
    assert 'JsonSerialize' not in source and 'JsonDeserialize' not in source
    assert operation['x-iroha-norito-request-type'] == 'iroha_torii_shared::ordinary_mint_finalized::OrdinaryMintFinalizedReadV1'
    accept = next(value for value in operation['parameters'] if value['name'] == 'Accept')
    assert accept['required'] is True
    assert accept['schema'] == {'type': 'string', 'const': 'application/x-norito'}
    assert set(operation['responses']['200']['content']) == {'application/x-norito'}
    assert 'content' not in operation['responses']['202']
    for status in ('200', '202'):
        assert operation['responses'][status]['headers']['Cache-Control']['schema']['const'] == 'private, no-store'
    for status in ('400', '401', '403', '429', '500', '503'):
        # The actual handlers return Error directly; unlike the four other reads,
        # they do not blanket-finalize error cache/nosniff/Vary headers.
        assert 'Cache-Control' not in operation['responses'][status].get('headers', {})
    finality = (ROOT / 'crates/iroha_data_model/src/sumeragi_finality.rs').read_text()
    model = (ROOT / 'crates/iroha_data_model/src/kagemusha/kagemusha_ordinary_mint_finality_v1.rs').read_text()
    mint = (ROOT / 'crates/iroha_data_model/src/kagemusha/kagemusha_v1.rs').read_text()
    assert 'MAX_FINALITY_BLOCK_BYTES: usize = 32 * 1024 * 1024' in finality
    assert 'KAGEMUSHA_OPERATION_RESULT_MAX_BYTES_V1 + 512 * 1024' in model
    assert 'KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1: usize = 7_936' in mint
    assert '+ super::KAGEMUSHA_MINT_CREDIT_MAX_BYTES_V1' in model and '+ 4096' in model


@pytest.mark.parametrize('path,method,stable_id,request_bound,response_bound,json_name', READS[:4])
def test_bridge_finalized_private_reads_keep_exact_outer_headers(document, path, method, stable_id, request_bound, response_bound, json_name):
    for response in document['paths'][path][method]['responses'].values():
        assert response['headers']['Cache-Control']['schema']['const'] == 'private, no-store'
        assert response['headers']['X-Content-Type-Options']['schema']['const'] == 'nosniff'
        assert response['headers']['Vary']['schema']['const'] == 'X-Iroha-Finality-Challenge, Accept'


@pytest.mark.parametrize('value', ['0' * 64, 'AB' * 32, 'ab' * 31, 'ab' * 33, 'ab' * 32 + ' ', '0x' + 'ab' * 32, 'gh' * 32])
def test_names_challenge_refuses_noncanonical_or_zero_http_spelling(document, value):
    operation = document['paths']['/v1/ledger/resource-names/{challenge}']['get']
    assert len(operation['parameters']) == 2
    for parameter in operation['parameters']:
        assert parameter['required'] is True
        check = Draft202012Validator(parameter['schema'])
        check.validate('ab' * 32)
        assert not check.is_valid(value)
    assert next(p for p in operation['parameters'] if p['in'] == 'header')['x-iroha-header-count'] == 1


@pytest.mark.parametrize('name', ['NativeNonzeroDigestV1', 'NativeSha256DigestV1'])
def test_fixed_byte_json_is_uppercase_hex_and_distinct_from_hash_literals(document, name):
    check = validator(document, name)
    check.validate('AB' * 32)
    assert check.is_valid('0' * 64) is (name == 'NativeSha256DigestV1')
    for invalid in ('ab' * 32, 'AB' * 31, 'AB' * 33, [171] * 32, 'hash:' + 'AB' * 32 + '#ABCD', '0x' + 'AB' * 32):
        assert not check.is_valid(invalid)


@pytest.mark.parametrize('name', ['NativeByteVectorV1', 'NativeRowByteVectorV1'])
def test_variable_bytes_keep_integer_array_json(document, name):
    check = validator(document, name)
    for valid in ([], [0, 255], [17, 35, 0]):
        check.validate(valid)
    for invalid in ('00FF', 'AP8=', [-1], [256], [True], [1.5], [None]):
        assert not check.is_valid(invalid)


def test_account_value_required_nulls_tuple_identifiers_and_full_stored_label(document, shape_data, subtests):
    check = validator(document, 'NativeAccountValueV1')
    hash_literal = shape_data['network_id']
    label = {'label': 'merchant', 'domain': ['bank'], 'dataspace': 3}
    for value in (
        {'metadata': {}, 'label': None, 'uaid': None, 'opaque_ids': []},
        {'metadata': {'public_shape': {'value': 1}}, 'label': label, 'uaid': [hash_literal], 'opaque_ids': [[hash_literal]]},
    ):
        check.validate(value)
        for field in value:
            with subtests.test(field=field):
                changed = copy.deepcopy(value)
                del changed[field]
                assert not check.is_valid(changed)
    for invalid in (
        {'metadata': {}, 'label': label, 'uaid': hash_literal, 'opaque_ids': []},
        {'metadata': {}, 'label': {'label': 'merchant', 'domain': 'bank', 'dataspace': 3}, 'uaid': None, 'opaque_ids': []},
        {'metadata': {}, 'label': None, 'uaid': None, 'opaque_ids': [hash_literal]},
        {'metadata': {}, 'label': None, 'uaid': None, 'opaque_ids': [], 'id': shape_data['signatory']},
    ):
        assert not check.is_valid(invalid)


@pytest.mark.parametrize('kind', ['direct', 'role'])
def test_issuer_grant_contains_complete_permission_payload_and_only_native_role_json(document, kind):
    permission = {'name': 'CanMintAsset', 'payload': {'asset': 'public-shape'}}
    grant = {'kind': kind, 'value': [permission] if kind == 'direct' else {'id': 'mint_role', 'permissions': [permission]}}
    check = validator(document, 'OrdinaryMintIssuerGrantOriginalV1')
    check.validate(grant)
    missing = copy.deepcopy(grant)
    tokens = missing['value'] if kind == 'direct' else missing['value']['permissions']
    del tokens[0]['payload']
    assert not check.is_valid(missing)
    for malformed in ({'kind': kind}, {'kind': kind.title(), 'value': grant['value']}, {'kind': kind, 'payload': grant['value']}, {**grant, 'unknown': 0}):
        assert not check.is_valid(malformed)
    if kind == 'role':
        changed = copy.deepcopy(grant)
        changed['value']['permission_epochs'] = {}
        assert not check.is_valid(changed)
    nullable = {'kind': kind, 'value': [{'name': 'CanReadAllLedgerData', 'payload': None}] if kind == 'direct' else {'id': 'read_role', 'permissions': [{'name': 'CanReadAllLedgerData', 'payload': None}]}}
    check.validate(nullable)


@pytest.mark.parametrize('kind', ['AccountAlias', 'GlobalFeeProgram'])
def test_authority_selector_requires_exact_case_and_payload_members(document, shape_data, kind):
    payload = {'label': 'merchant', 'domain': None, 'dataspace': 'retail'} if kind == 'AccountAlias' else {
        'program_id': {'sponsor': shape_data['signatory'], 'name': 'fees'}, 'fee_asset': shape_data['asset_definition']['id']}
    selector = {'kind': kind, 'payload': payload}
    check = validator(document, 'NativeAuthorityOriginalsSelectorV1')
    check.validate(selector)
    for malformed in ({'kind': kind.lower(), 'payload': payload}, {'kind': kind, 'value': payload}, {'kind': kind}, {**selector, 'table': 'world.accounts'}):
        assert not check.is_valid(malformed)


def test_native_alias_state_requires_selected_even_when_absent(document):
    value = {'alias': {'label': 'merchant', 'domain': None, 'dataspace': 3}, 'binding_keys': [], 'selected': None}
    check = validator(document, 'NativeAccountAliasStateV1')
    check.validate(value)
    del value['selected']
    assert not check.is_valid(value)


def test_rekey_history_preserves_required_signatory_and_unit_provenance(document, shape_data):
    value = {'label': {'label': 'merchant', 'domain': None, 'dataspace': 3}, 'active_account_id': shape_data['wallet'],
        'previous_account_ids': [shape_data['signatory']], 'active_signatory': None, 'previous_signatories': [],
        'transition_provenance': [{'kind': 'alias_reassignment', 'value': None}]}
    check = validator(document, 'NativeAccountRekeyRecordV1')
    check.validate(value)
    for field in ('active_signatory', 'transition_provenance'):
        changed = copy.deepcopy(value)
        del changed[field]
        assert not check.is_valid(changed)
    for wrong in ({'kind': 'alias_reassignment'}, {'kind': 'AliasReassignment', 'value': None}, {'kind': 'account_id_rekey', 'value': 0}):
        changed = copy.deepcopy(value)
        changed['transition_provenance'] = [wrong]
        assert not check.is_valid(changed)


@pytest.mark.parametrize('schema,path', [
    ('NativeResourceNamesStateV1', 'crates/iroha_torii_shared/src/resource_names_state.rs'),
    ('NativeAuthorityOriginalsV1', 'crates/iroha_torii_shared/src/authority_originals.rs'),
    ('NativeAuthorityOriginalsRequestV1', 'crates/iroha_torii_shared/src/authority_originals.rs'),
    ('OrdinaryWalletCurrentRequestV1', 'crates/iroha_torii_shared/src/ordinary_wallet_current.rs'),
    ('OrdinaryWalletCurrentOriginalV1', 'crates/iroha_torii_shared/src/ordinary_wallet_current.rs'),
    ('OrdinaryMintIssuerPurposeRequestV1', 'crates/iroha_torii_shared/src/ordinary_mint_issuer_purpose.rs'),
    ('OrdinaryMintIssuerPurposeOriginalV1', 'crates/iroha_torii_shared/src/ordinary_mint_issuer_purpose.rs'),
    ('NativeAccountAliasStateV1', 'crates/iroha_torii_shared/src/authority_originals.rs'),
    ('NativeAccountAliasOriginalV1', 'crates/iroha_torii_shared/src/authority_originals.rs'),
    ('NativeGlobalFeeProgramStateV1', 'crates/iroha_torii_shared/src/authority_originals.rs'),
])
def test_complete_native_struct_fields_are_required_and_not_query_projections(document, schema, path):
    source = (ROOT / path).read_text()
    body = source.split('pub struct ' + schema + ' {', 1)[1].split('\n}', 1)[0]
    fields = set(re.findall(r'(?m)^    pub ([a-z_][a-z_0-9]*):', body))
    actual = document['components']['schemas'][schema]
    assert set(actual['properties']) == set(actual['required']) == fields
    assert actual['additionalProperties'] is False


@pytest.mark.parametrize('name', ['NativeResourceNamesStateV1', 'OrdinaryWalletCurrentOriginalV1', 'OrdinaryMintIssuerPurposeOriginalV1', 'NativeAuthorityOriginalsV1'])
def test_complete_response_shapes_reject_missing_root_and_unknown_members(document, shape_data, name):
    base = {'attestation': shape_data['attestation'], 'world_snapshot': shape_data['world_snapshot']}
    account = {'metadata': {}, 'label': None, 'uaid': None, 'opaque_ids': []}
    if name == 'NativeResourceNamesStateV1':
        value = {**base, 'asset_alias_bindings': [{'definition_id': shape_data['asset_definition']['id'], 'binding_record_wire': [0, 255]}],
            'smart_contract_keys': ['public_fixture/path'], 'dataspace_names': [{'storage_key': 'public_fixture/path', 'raw_value': [17, 35]}]}
    elif name == 'OrdinaryWalletCurrentOriginalV1':
        request = {'version': 1, 'network_id': shape_data['network_id'], 'height': 2, 'request_nonce': '07' * 32,
            'signatory': shape_data['signatory'], 'wallet': shape_data['wallet']}
        value = {**base, 'request': request, 'signatory_value': account, 'wallet_value': copy.deepcopy(account)}
    elif name == 'OrdinaryMintIssuerPurposeOriginalV1':
        token = {'name': 'CanMintAsset', 'payload': {'asset': shape_data['asset_definition']['id']}}
        request = {'version': 1, 'network_id': shape_data['network_id'], 'height': 2, 'request_nonce': '07' * 32,
            'issuer': shape_data['signatory'], 'asset': shape_data['asset_definition']['id'], 'purpose': token}
        value = {**base, 'request': request, 'issuer_value': account, 'grant': {'kind': 'direct', 'value': [token]}}
    else:
        selector = {'kind': 'AccountAlias', 'payload': {'label': 'merchant', 'domain': None, 'dataspace': 'retail'}}
        value = {**base, 'request_sha256': 'AB' * 32, 'selector': selector,
            'originals': {'kind': 'AccountAlias', 'payload': {'alias': {'label': 'merchant', 'domain': None, 'dataspace': 3}, 'binding_keys': [], 'selected': None}}}
    check = validator(document, name)
    check.validate(value)
    for field in value:
        changed = copy.deepcopy(value)
        del changed[field]
        assert not check.is_valid(changed)
    changed = copy.deepcopy(value)
    changed['authority_granted'] = True
    assert not check.is_valid(changed)


@pytest.mark.parametrize('name', ['NativeFeeSponsorProgramRevisionKeyV1', 'NativeFeeSponsorEnrollmentKeyV1', 'NativeFeeSponsorVaultKeyV1', 'NativeFeeSponsorBudgetCounterKeyV1'])
def test_all_four_fee_keys_are_complete_named_objects(document, shape_data, name):
    program = {'sponsor': shape_data['signatory'], 'name': 'fees'}
    value = {'program_id': program}
    if name.endswith('ProgramRevisionKeyV1'):
        value['revision'] = 1
    elif name.endswith('EnrollmentKeyV1'):
        value['beneficiary'] = shape_data['wallet']
    else:
        value['asset_definition_id'] = shape_data['asset_definition']['id']
        if name.endswith('BudgetCounterKeyV1'):
            value['window'] = {'kind': 'block', 'value': {'height': 2}}
    check = validator(document, name)
    check.validate(value)
    for field in value:
        changed = copy.deepcopy(value)
        del changed[field]
        assert not check.is_valid(changed)
    assert not check.is_valid(list(value.values()))
    assert not check.is_valid({**value, 'unknown': 0})


@pytest.mark.parametrize('kind', ['block', 'program_epoch', 'beneficiary_epoch'])
def test_fee_budget_windows_keep_exact_tagged_payload(document, shape_data, kind):
    payload = {'height': 2} if kind == 'block' else {'epoch': 0}
    if kind == 'beneficiary_epoch':
        payload['beneficiary'] = shape_data['wallet']
    value = {'kind': kind, 'value': payload}
    check = validator(document, 'NativeFeeSponsorBudgetWindowV1')
    check.validate(value)
    for invalid in ({'kind': kind}, {'kind': kind.title(), 'value': payload}, {'kind': kind, 'value': None}, {'kind': kind, 'payload': payload}):
        assert not check.is_valid(invalid)


@pytest.mark.parametrize('kind', ['native_instruction', 'multisig', 'contract_call', 'ivm', 'ivm_proved'])
def test_native_fee_rule_selectors_preserve_all_actual_families(document, shape_data, kind):
    if kind == 'native_instruction':
        payload = {'wire_id': 'public_shape_instruction'}
    elif kind == 'multisig':
        payload = {'operations': [{'operation': 'approve', 'value': None}], 'account_ids': [shape_data['wallet']]}
    elif kind == 'contract_call':
        # A shape string, not a native contract-admission or checksum fixture.
        payload = {'contract_address': 'public_contract_shape', 'code_hash': shape_data['network_id'], 'entrypoints': ['main']}
    else:
        payload = {'code_hash': shape_data['network_id']}
    check = validator(document, 'NativeFeeSponsorRuleSelectorV1')
    value = {'kind': kind, 'value': payload}
    check.validate(value)
    for field in payload:
        changed = copy.deepcopy(value)
        del changed['value'][field]
        assert not check.is_valid(changed)
    assert not check.is_valid({'kind': kind.title(), 'value': payload})
    assert not check.is_valid({**value, 'unknown': 0})


def test_native_fee_program_omits_absent_option_fields_instead_of_null(document, shape_data):
    value = {'id': {'sponsor': shape_data['signatory'], 'name': 'fees'}, 'payout_account': shape_data['wallet'],
        'lifecycle': {'state': 'staged', 'value': None}}
    check = validator(document, 'NativeFeeSponsorProgramV1')
    check.validate(value)
    for field in ('active_revision', 'staged_revision', 'scheduled_activation'):
        assert not check.is_valid({**value, field: None})
    check.validate({**value, 'active_revision': 1, 'staged_revision': 2, 'scheduled_activation': {'revision': 2, 'activate_at_height': 3}})
    changed = copy.deepcopy(value)
    del changed['lifecycle']['value']
    assert not check.is_valid(changed)


def test_native_fee_family_retains_complete_rows_and_required_null_program(document, shape_data, subtests):
    program = {'sponsor': shape_data['signatory'], 'name': 'fees'}
    asset = shape_data['asset_definition']['id']
    key = {'program_id': program, 'asset_definition_id': asset}
    value = {'program_id': program, 'fee_asset': asset,
        'asset_keys': [asset + '#' + shape_data['signatory']], 'program_keys': [program],
        'revision_keys': [{'program_id': program, 'revision': 1}],
        'enrollment_keys': [{'program_id': program, 'beneficiary': shape_data['wallet']}],
        'vault_keys': [key], 'budget_counter_keys': [{**key, 'window': {'kind': 'block', 'value': {'height': 2}}}],
        'sponsor_account_value': {'metadata': {}, 'label': None, 'uaid': None, 'opaque_ids': []},
        'fee_asset_definition': shape_data['asset_definition'], 'source_asset_value': '0', 'program': None,
        'revisions': [{'program_id': program, 'revision': 1, 'eligibility': {'mode': 'enrolled_only', 'value': None},
            'rules': [{'id': 'default_fee', 'effect': {'effect': 'allow', 'value': None},
                'selectors': [{'kind': 'ivm', 'value': {'code_hash': shape_data['network_id']}}]}],
            'asset_budgets': [{'asset_definition_id': asset, 'per_transaction': '1', 'per_block': '2', 'per_program_epoch': '3',
                'per_beneficiary_epoch': '1', 'reserve_floor': '0', 'epoch_length_blocks': 1}]}],
        'enrollments': [{'key': {'program_id': program, 'beneficiary': shape_data['wallet']}, 'enrolled_at_height': 2}],
        'vaults': [{'key': key, 'balance': '1'}]}
    check = validator(document, 'NativeGlobalFeeProgramStateV1')
    check.validate(value)
    for field in value:
        with subtests.test(field=field):
            changed = copy.deepcopy(value)
            del changed[field]
            assert not check.is_valid(changed)
    for wrong_quantity in (0, -1, '-1', '01', '1.0'):
        changed = copy.deepcopy(value)
        changed['source_asset_value'] = wrong_quantity
        assert not check.is_valid(changed)
    changed = copy.deepcopy(value)
    changed['revisions'][0]['asset_budgets'][0]['epoch_length_blocks'] = 0
    assert not check.is_valid(changed)
    changed = copy.deepcopy(value)
    changed['vaults'][0]['unknown'] = 0
    assert not check.is_valid(changed)
    changed = copy.deepcopy(value)
    changed['program_keys'] = [program] * 16385
    assert not check.is_valid(changed)


@pytest.mark.parametrize('name', ['NativeNameV1', 'NativeStatePathV1'])
def test_native_text_shapes_refuse_forbidden_characters_and_expose_byte_bounds(document, name):
    check = validator(document, name)
    for valid in ('native_name', 'é', 'public_fixture/path'):
        check.validate(valid)
    for invalid in ('', 'a b', 'a@b', 'a#b', 'a$b', 'a\x00b', 'a\u061cb', 'a\u202eb'):
        assert not check.is_valid(invalid)
    expected = 255 if name == 'NativeNameV1' else 16384
    assert document['components']['schemas'][name]['x-iroha-maxUtf8Bytes'] == expected
    assert not check.is_valid('a' * (expected + 1))
