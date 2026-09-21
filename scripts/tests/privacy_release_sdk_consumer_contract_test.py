"""Closed qualification consumer schema controls; these do not qualify SDK execution."""

from pathlib import Path

import json
import pytest
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[2]
AUTHORITIES = (
    "artifacts/openapi/torii.json",
    "artifacts/openapi/versions/current/torii.json",
    "crates/iroha_torii/assets/openapi/torii.json",
)
CONSUMERS = (
    "kotlin_jvm", "kotlin_android", "java_source_kotlin", "swift_c_bridge",
    "javascript_napi", "python_pyo3", "csharp", "cli", "openapi", "genesis_tooling",
)


@pytest.fixture(params=AUTHORITIES)
def document(request):
    return json.loads((ROOT / request.param).read_text(encoding="utf-8"))


def validator(document, schema):
    """Resolve the actual authored component references without copying their contents."""
    return Draft202012Validator({**schema, "components": document["components"]})


def test_sdk_consumer_mirrors_are_identical():
    assert len({(ROOT / path).read_bytes() for path in AUTHORITIES}) == 1


def test_sdk_consumer_inventory_retains_exact_ten_and_both_java_platforms(document):
    schema = document["components"]["schemas"]["PrivacyReleaseSdkConsumerV1"]
    assert schema["properties"]["consumer"]["enum"] == list(CONSUMERS)
    assert schema["required"] == ["consumer", "value"]
    assert schema["additionalProperties"] is False
    for phrase in ("Java-source", "canonical Kotlin APIs", "both JVM and Android"):
        assert phrase in schema["description"]


@pytest.mark.parametrize("tag", CONSUMERS)
def test_sdk_consumer_accepts_each_canonical_identity(document, tag):
    schema = {"$ref": "#/components/schemas/PrivacyReleaseSdkConsumerV1"}
    validator(document, schema).validate({"consumer": tag, "value": None})


@pytest.mark.parametrize("payload", [
    {"consumer": "java_android", "value": None},
    {"consumer": "JavaAndroid", "value": None},
    {"consumer": "JavaSourceKotlin", "value": None},
    {"consumer": "java_source_kotlin"},
    {"consumer": "java_source_kotlin", "value": {}},
    {"consumer": "java_source_kotlin", "value": None, "platform": "android"},
])
def test_sdk_consumer_rejects_retired_alias_and_noncanonical_shapes(document, payload):
    schema = {"$ref": "#/components/schemas/PrivacyReleaseSdkConsumerV1"}
    assert not validator(document, schema).is_valid(payload)


def test_sdk_package_inventory_requires_ten_rows_and_current_java_identity(document):
    schemas = document["components"]["schemas"]
    schema = schemas["PrivacyExact12ReleaseManifestV1"]["properties"]["sdk_packages"]
    assert schema["minItems"] == schema["maxItems"] == 10
    packages = [
        {
            "consumer": {"consumer": tag, "value": None},
            "package_name": f"fixture-{tag}",
            "package_version": "1.0.0",
            "package_digest": [index + 1] * 32,
            "fixture_corpus_digest": [31] * 32,
        }
        for index, tag in enumerate(CONSUMERS)
    ]
    check = validator(document, schema)
    check.validate(packages)
    assert not check.is_valid(packages[:2] + packages[3:])
    assert not check.is_valid(packages + [packages[2]])
    packages[2]["consumer"]["consumer"] = "java_android"
    assert not check.is_valid(packages)
    # Native validation separately enforces canonical row order and distinct package digests.
