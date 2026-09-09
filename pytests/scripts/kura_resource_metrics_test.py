"""Synthetic parser controls only; no endpoint, timing, RSS or scaling evidence."""
from __future__ import annotations

import dataclasses
import hashlib
import importlib.util
import json
from pathlib import Path
import sys

import pytest

PRIVATE = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("private_kura_resource_metrics", PRIVATE / "scripts/nexus/kura_resource_metrics.py")
parser = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = parser
spec.loader.exec_module(parser)
P = "iroha_kura_resource_"
FIELDS = ("resident_associations", "persisted_entries", "index_bytes", "temporary_index_bytes", "storage_bytes")
FAMILIES = (
    "resident_canonical", "resident_transaction", "resident_merge", "resident_carrier",
    "resident_replica", "resident_verification", "resident_frontier", "resident_queue",
    "canonical_index", "canonical_hashes", "pipeline_index", "ownership_index", "certified_index",
    "execution_input_index", "execution_preflight_index", "application_receipt_index",
    "merge_bundle_index", "canonical_replica_index", "merge_carrier_record", "native_latest_record",
    "query_marker_records", "evidence_key_records", "storage_bytes",
)
REASONS = ("owner_unavailable", "unregistered", "busy", "interrupted", "arithmetic", "generation_changed", "owner_mismatch", "invalid_inventory", "numeric_range")


def success() -> bytes:
    rows = [P + 'available 1', P + 'status{reason="available"} 1', P + 'generation 17', P + 'fault_count 2']
    totals = [0] * 5
    for ordinal, family in enumerate(FAMILIES, start=1):
        values = (ordinal if ordinal <= 8 else 0, ordinal if 8 < ordinal < 23 else 0,
                  32 + 16 * ordinal if 8 < ordinal < 23 else 0, 0,
                  8192 if ordinal == 23 else 0)
        for column, (field, value) in enumerate(zip(FIELDS, values, strict=True)):
            rows.append(f'{P}{field}{{family="{family}"}} {value}')
            totals[column] += value
    rows.extend(f'{P}{field}_sum {value}' for field, value in zip(FIELDS, totals, strict=True))
    rows.append(f'{P}represented_entries {totals[0] + totals[1]}')
    assert len(rows) == 125
    return ('\n'.join(rows) + '\n').encode()


def missing(reason="busy") -> bytes:
    return f'{P}available 0\n{P}status{{reason="{reason}"}} 1\n'.encode()


def replace_once(raw: bytes, old: str, new: str) -> bytes:
    needle = old.encode()
    assert raw.count(needle) == 1
    return raw.replace(needle, new.encode())


def generation(token: str) -> bytes:
    return replace_once(success(), P + 'generation 17\n', P + f'generation {token}\n')


def test_projection_matches_frozen_owner_and_typed_complete_response():
    review = PRIVATE / "pytests/fixtures/kura_resource_metric_projection_v1.json"
    raw_projection = review.read_bytes()
    assert hashlib.sha256(raw_projection).hexdigest() == parser.REVIEWED_PROJECTION_SHA256
    projection = json.loads(raw_projection)
    assert {row['name'] for row in projection['metric_families']} == parser.METRIC_NAMES
    assert tuple(projection['family_labels']) == parser.FAMILIES == FAMILIES
    assert tuple(projection['unavailable']['reason_labels']) == REASONS
    raw = success()
    observation = parser.parse_kura_resource_metrics(raw)
    assert isinstance(observation, parser.AvailableObservation)
    assert observation.available is True
    assert observation.raw_sha256 == hashlib.sha256(raw).hexdigest()
    assert observation.response_bytes == len(raw)
    assert observation.generation == 17
    assert observation.fault_count == 2
    assert len(observation.components) == 23
    assert tuple(component.family for component in observation.components) == FAMILIES
    assert observation.total.resident_associations == sum(range(1, 9))
    assert observation.total.persisted_entries == sum(range(9, 23))
    assert observation.total.index_bytes == sum(32 + 16 * i for i in range(9, 23))
    assert observation.total.temporary_index_bytes == 0
    assert observation.total.storage_bytes == 8192
    assert observation.represented_entries == sum(range(1, 23))
    with pytest.raises(dataclasses.FrozenInstanceError):
        observation.generation = 0


@pytest.mark.parametrize('reason', REASONS)
def test_unavailable_is_exact_two_rows_and_has_no_numeric_inventory(reason):
    raw = missing(reason)
    result = parser.parse_kura_resource_metrics(raw)
    assert isinstance(result, parser.UnavailableObservation)
    assert result.available is False
    assert result.reason.value == reason
    assert result.raw_sha256 == hashlib.sha256(raw).hexdigest()
    assert result.response_bytes == len(raw)
    assert set(dataclasses.asdict(result)) == {'raw_sha256', 'response_bytes', 'reason'}
    for name in ('generation', 'fault_count', 'components', 'total', 'represented_entries'):
        assert not hasattr(result, name)


@pytest.mark.parametrize('token,value', [
    ('0', 0), ('+0.000e+3', 0), ('0e1024', 0), ('0e-1024', 0),
    ('17.', 17), ('+17.000', 17), ('.17e2', 17), ('17000e-3', 17),
    ('0001700.000e-2', 17), ('1E+000', 1),
    ('9007199254740991', 2**53-1), ('9007199254740992', 2**53),
    ('9.007199254740992e15', 2**53), ('9007199254740992000e-3', 2**53),
])
def test_exact_finite_scientific_decimal_values(token, value):
    assert parser.parse_kura_resource_metrics(generation(token)).generation == value


def test_every_sample_accepts_exact_decimal_without_float_rounding():
    rows = []
    for line in success().decode().splitlines():
        key, value = line.rsplit(' ', 1)
        rows.append(f'{key} +{value}000.000e-3')
    result = parser.parse_kura_resource_metrics(('\n'.join(reversed(rows)) + '\n').encode())
    baseline = parser.parse_kura_resource_metrics(success())
    assert dataclasses.replace(result, raw_sha256=baseline.raw_sha256, response_bytes=baseline.response_bytes) == baseline


@pytest.mark.parametrize('token', [
    'NaN', 'nan', '+Inf', '-Inf', 'Infinity', 'true', 'False', '',
    '-1', '-0', '-0.0', '0.1', '17.000000000000001', '1e-1024',
    '9007199254740993', '9.007199254740993e15', '9007199254740992999e-3',
    '1e1024', '1e1025', '0e1025', '0e-1025', '1e' + '9'*100,
    '0'*129, '1_' + '0', '0x10', '١٧', '17 123456', '17 # exemplar',
    '17\t123', '17\u00a0123', '\x0017', '17\rgarbage',
])
def test_numeric_attacks_and_ambiguous_sample_fields_are_rejected(token):
    with pytest.raises(parser.ProjectionError):
        parser.parse_kura_resource_metrics(generation(token))


@pytest.mark.parametrize('offset', range(125))
def test_each_required_success_sample_is_mandatory(offset):
    rows = success().splitlines(keepends=True)
    rows.pop(offset)
    with pytest.raises(parser.ProjectionError):
        parser.parse_kura_resource_metrics(b''.join(rows))


@pytest.mark.parametrize('offset', range(125))
def test_each_target_sample_cannot_be_duplicated(offset):
    rows = success().splitlines(keepends=True)
    with pytest.raises(parser.ProjectionError, match='duplicate target sample'):
        parser.parse_kura_resource_metrics(success() + rows[offset])


@pytest.mark.parametrize('extra', [
    'generation 1', 'fault_count 0', 'storage_bytes_sum 0', 'represented_entries 0',
    'index_bytes{family="canonical_index"} 0', 'status{reason="interrupted"} 1',
])
def test_unavailable_cannot_carry_any_stale_numeric_vector(extra):
    with pytest.raises(parser.ProjectionError):
        parser.parse_kura_resource_metrics(missing() + (P + extra + '\n').encode())


@pytest.mark.parametrize('raw', [
    b'', b'other_metric 1\n',
    (P+'available 2\n'+P+'status{reason="available"} 1\n').encode(),
    (P+'available 1\n'+P+'status{reason="busy"} 1\n').encode(),
    (P+'available 0\n'+P+'status{reason="available"} 1\n').encode(),
    (P+'available 0\n'+P+'status{reason="busy"} 0\n').encode(),
    (P+'available 0\n'+P+'status{reason="unknown"} 1\n').encode(),
])
def test_status_discriminant_is_closed_and_consistent(raw):
    with pytest.raises(parser.ProjectionError):
        parser.parse_kura_resource_metrics(raw)


@pytest.mark.parametrize('replacement', [
    'resident_associations{family="unknown"} 1',
    'resident_associations{other="resident_canonical"} 1',
    'resident_associations{family="resident_canonical",family="resident_canonical"} 1',
    'resident_associations{family="resident_canonical",extra="x"} 1',
    'resident_associations{} 1', 'resident_associations 1',
    'resident_associations{family=resident_canonical} 1',
    'resident_associations{family="resident_canonical" 1',
    'resident_associations{family="resident_canonical"}1',
    'resident_associations{family="resident_canonical"}; 1',
    'resident_associations{family="resident_canonical"} 1 #foo',
    'resident_associations{family="resident\\_canonical"} 1',
    'resident_associations{family="resident\\ncanonical"} 1',
    'resident_associations{family="resident\\\\canonical"} 1',
    'resident_associations{family="resident\\\"canonical"} 1',
    'resident_associations{family="\\u0072esident_canonical"} 1',
    'resident_associations{family="resident_canonical\x00"} 1',
    'resident_associations{family="' + 'x'*257 + '"} 1',
    'resident_associations{family="' + 'x'*129 + '"} 1',
    'resident_associations{family="resident_canonical\\"} 1',
])
def test_owned_label_grammar_and_escaping_cannot_hide_bad_target_rows(replacement):
    raw = replace_once(success(), P+'resident_associations{family="resident_canonical"} 1\n', P+replacement+'\n')
    with pytest.raises(parser.ProjectionError):
        parser.parse_kura_resource_metrics(raw)


@pytest.mark.parametrize('extra', [
    'unknown_metric 0', 'generation_sum 0', 'available{family="canonical_index"} 1',
    'status{reason="\\u0062usy"} 1', 'status{reason="busy\\n"} 1',
    'storage_bytes{family="STORAGE_BYTES"} 0', 'index_bytes_sum_extra 0',
])
def test_unknown_target_names_and_scalar_labels_are_never_ignored(extra):
    with pytest.raises(parser.ProjectionError):
        parser.parse_kura_resource_metrics(success() + (P + extra + '\n').encode())


@pytest.mark.parametrize('field', FIELDS)
def test_each_subtotal_is_independently_reconciled(field):
    raw = success()
    old = next(line for line in raw.decode().splitlines() if line.startswith(P + field + '_sum '))
    damaged = replace_once(raw, old+'\n', old.rsplit(' ', 1)[0]+' 9007199254740992\n')
    with pytest.raises(parser.ProjectionError, match='inconsistent component subtotal'):
        parser.parse_kura_resource_metrics(damaged)


def test_represented_sum_and_overflowing_component_aggregate_fail():
    raw = replace_once(success(), P+'represented_entries 253\n', P+'represented_entries 254\n')
    with pytest.raises(parser.ProjectionError, match='represented-entry'):
        parser.parse_kura_resource_metrics(raw)
    raw = success()
    for family, old in [('resident_canonical', 1), ('resident_transaction', 2)]:
        raw = replace_once(raw, f'{P}resident_associations{{family="{family}"}} {old}\n', f'{P}resident_associations{{family="{family}"}} {2**53}\n')
    raw = replace_once(raw, P+'resident_associations_sum 36\n', P+f'resident_associations_sum {2**53}\n')
    with pytest.raises(parser.ProjectionError, match='component subtotal'):
        parser.parse_kura_resource_metrics(raw)


def test_unrelated_namespaces_are_ignored_but_raw_provenance_is_preserved():
    extra = b'other NaN\nother{broken label <<\n# HELP other iroha_kura_resource_generation\nother{label="iroha_kura_resource_fake"} 999\n'
    raw = extra + success().replace(b'\n', b'\r\n')
    result = parser.parse_kura_resource_metrics(raw)
    assert result.generation == 17
    assert result.raw_sha256 == hashlib.sha256(raw).hexdigest()
    assert result.response_bytes == len(raw)
    assert result.raw_sha256 != parser.parse_kura_resource_metrics(success()).raw_sha256


def test_optional_target_metadata_is_validated_without_becoming_a_sample():
    prefix = f'# HELP {P}available Current availability.\n# TYPE {P}available gauge\n'
    assert parser.parse_kura_resource_metrics(prefix.encode() + success()).generation == 17
    with pytest.raises(parser.ProjectionError, match='duplicate target metadata'):
        parser.parse_kura_resource_metrics((prefix+prefix).encode() + success())
    for metadata in [f'# TYPE {P}available counter\n', f'# TYPE {P}available\n',
                     f'# HELP {P}unknown x\n', f'# {P}generation 1\n']:
        with pytest.raises(parser.ProjectionError):
            parser.parse_kura_resource_metrics(metadata.encode() + success())


def test_all_hard_bounds_apply_before_large_allocations_or_ignored_namespace_work():
    for raw in [b'x'*(parser.MAX_RESPONSE_BYTES+1),
                b'#'*(parser.MAX_LINE_BYTES+1)+b'\n'+success(),
                b'\n'*parser.MAX_RESPONSE_LINES+success(),
                b'other \xff\n'+success()]:
        with pytest.raises(parser.ProjectionError):
            parser.parse_kura_resource_metrics(raw)
    # A line exactly at the content byte ceiling remains valid; unrelated syntax
    # is not interpreted by the resource parser.
    raw = b'x'*parser.MAX_LINE_BYTES+b'\r\n'+missing()
    assert isinstance(parser.parse_kura_resource_metrics(raw), parser.UnavailableObservation)
    for value in [success().decode(), bytearray(success()), None, True]:
        with pytest.raises(parser.ProjectionError, match='immutable response bytes'):
            parser.parse_kura_resource_metrics(value)


@pytest.mark.parametrize('line', [
    '# UNIT ' + P + 'storage_bytes bytes',
    '# UNKNOWN ' + P + 'unknown 1',
    '# TYPE ' + P + 'available\x0bgauge',
    P + 'x'*128 + ' 1',
    '# TYPE ' + P + 'x'*128 + ' gauge',
])
def test_unknown_owned_metadata_and_oversized_metric_tokens_fail(line):
    with pytest.raises(parser.ProjectionError):
        parser.parse_kura_resource_metrics((line+'\n').encode() + success())


def test_distinct_extra_valid_reason_exceeds_success_row_bound():
    extra = (P + 'status{reason="busy"} 1\n').encode()
    with pytest.raises(parser.ProjectionError, match='target sample count exceeds bound'):
        parser.parse_kura_resource_metrics(success() + extra)


def test_label_token_bound_includes_closing_quote():
    # Lexical control only: the exact bound permits 127 escaped backslashes;
    # adding one plain byte must fail when the final quote is counted.
    boundary = '"' + (chr(92) * 2) * 127 + '"'
    assert len(boundary.encode()) == parser.MAX_LABEL_TOKEN_BYTES
    assert parser._quoted(boundary, 0, 1) == (chr(92) * 127, len(boundary))
    quoted = boundary[:-1] + 'a"'
    assert len(quoted.encode()) == parser.MAX_LABEL_TOKEN_BYTES + 1
    with pytest.raises(parser.ProjectionError, match='label token exceeds bound'):
        parser._quoted(quoted, 0, 1)


def test_whitespace_and_empty_scalar_labels_do_not_create_distinct_series():
    raw = replace_once(success(), P+'available 1\n', '  '+P+'available{}\t+1.0\n')
    raw = replace_once(raw, P+'resident_associations{family="resident_canonical"} 1\n',
                       P+'resident_associations{ family = "resident_canonical", }\t1\n')
    assert parser.parse_kura_resource_metrics(raw).generation == 17
    with pytest.raises(parser.ProjectionError, match='duplicate target sample'):
        parser.parse_kura_resource_metrics(raw + (P+'available 1\n').encode())
