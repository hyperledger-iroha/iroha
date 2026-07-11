from __future__ import annotations

import json
import re
from copy import deepcopy
from pathlib import Path
from typing import Any, Dict

import pytest

import iroha_python.client as client_module

from iroha_python import (
    ContractEntrypointKind,
    ContractManifest,
    ContractManifestRecord,
    EntrypointValueKindV1,
    EntrypointValueTypeNodeKindV1,
    EntrypointValueTypeV1,
)


_QUERY_VIEW_LAYOUTS = {
    "AccountView": (
        ["id", "metadata"],
        ["AccountId", "Json"],
    ),
    "AssetView": (
        ["id", "amount"],
        ["AssetId", "Amount"],
    ),
    "AssetDefinitionView": (
        [
            "id",
            "name",
            "description",
            "owned_by",
            "total_quantity",
            "metadata",
        ],
        [
            "AssetDefinitionId",
            "String",
            ("Option", "String"),
            "AccountId",
            "Amount",
            "Json",
        ],
    ),
    "DomainView": (
        ["id", "owned_by", "metadata"],
        ["DomainId", "AccountId", "Json"],
    ),
    "NftView": (
        ["id", "owned_by", "content"],
        ["NftId", "AccountId", "Json"],
    ),
}


def _leaf_node(kind: str) -> Dict[str, Any]:
    return {"kind": "Leaf", "value": {"kind": kind, "value": None}}


def _query_view_nodes(name: str) -> list[Dict[str, Any]]:
    fields, children = _QUERY_VIEW_LAYOUTS[name]
    nodes: list[Dict[str, Any]] = [
        {"kind": "Struct", "value": {"name": name, "fields": fields}}
    ]
    for child in children:
        if isinstance(child, tuple):
            wrapper, leaf = child
            nodes.extend(({"kind": wrapper, "value": None}, _leaf_node(leaf)))
        else:
            nodes.append(_leaf_node(child))
    return nodes


def _query_page_payload(name: str) -> Dict[str, Any]:
    return {
        "nodes": [
            {
                "kind": "Struct",
                "value": {"name": "QueryPage", "fields": ["items", "next_offset"]},
            },
            {"kind": "List", "value": {"capacity": 64}},
            *_query_view_nodes(name),
            {"kind": "Option", "value": None},
            _leaf_node("Int"),
        ]
    }


def _full_manifest_payload() -> Dict[str, Any]:
    return {
        "seiyaku_name": "Ledger",
        "code_hash": "hash:BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB#ABA2",
        "abi_hash": "hash:DDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD#F071",
        "compiler_fingerprint": "kotodama_lang",
        "features_bitmap": 0,
        "access_set_hints": {
            "read_keys": ["state:Balances"],
            "write_keys": ["state:Balances"],
            "dynamic_reads": [
                {
                    "base_key": "state:Balances",
                    "key_type": "AccountId",
                    "bound_kind": "take",
                    "max_keys": 64,
                }
            ],
            "dynamic_writes": [],
        },
        "entrypoints": [
            {
                "name": "transfer",
                "kind": {"kind": "Kotoage", "value": None},
                "params": [
                    {"name": "request", "type_name": "struct Transfer"},
                    {"name": "tags", "type_name": "List<Name, 64>"},
                ],
                "argument_schema": {
                    "fields": [
                        {
                            "name": "request",
                            "ty": {
                                "nodes": [
                                    {
                                        "kind": "Struct",
                                        "value": {
                                            "name": "Transfer",
                                            "fields": ["amount", "memo"],
                                        },
                                    },
                                    {
                                        "kind": "Leaf",
                                        "value": {"kind": "Amount", "value": None},
                                    },
                                    {"kind": "Option", "value": None},
                                    {
                                        "kind": "Leaf",
                                        "value": {"kind": "String", "value": None},
                                    },
                                ]
                            },
                        },
                        {
                            "name": "tags",
                            "ty": {
                                "nodes": [
                                    {
                                        "kind": "List",
                                        "value": {"capacity": 64},
                                    },
                                    {
                                        "kind": "Leaf",
                                        "value": {
                                            "kind": "Name",
                                            "value": None,
                                        },
                                    },
                                ]
                            },
                        },
                    ]
                },
                "return_type": "Result<(bool, u128), string>",
                "return_schema": {
                    "nodes": [
                        {"kind": "Result", "value": None},
                        {"kind": "Tuple", "value": 2},
                        {
                            "kind": "Leaf",
                            "value": {"kind": "Bool", "value": None},
                        },
                        {
                            "kind": "Leaf",
                            "value": {"kind": "U128", "value": None},
                        },
                        {
                            "kind": "Leaf",
                            "value": {"kind": "String", "value": None},
                        },
                    ]
                },
                "permission": "TransferAsset",
                "read_keys": ["state:Balances"],
                "write_keys": ["state:Balances"],
                "access_hints_complete": True,
                "access_hints_skipped": [],
                "triggers": [
                    {
                        "id": "settle",
                        "repeats": {"Exactly": 2},
                        "filter": "TlJUMAAAl9+YQQ4oJZjALRf6FAto0QAKAAAAAAAAANzCjydU9+jNAgIAAAAFBAAAAAA=",
                        "authority": None,
                        "metadata": {"purpose": "daily-settlement", "round": 7},
                        "callback": {"namespace": None, "entrypoint": "transfer"},
                    }
                ],
            }
        ],
        "states": [{"name": "Balances", "type_name": "StateMap<AccountId, Amount>"}],
        "error_codes": [
            {"namespace": "TransferError", "name": "InsufficientFunds", "code": 1001}
        ],
        "kotoba": [
            {
                "msg_id": "transfer.denied",
                "translations": [
                    {"lang": "en", "text": "Transfer denied"},
                    {"lang": "ja", "text": "送金は拒否されました"},
                ],
            }
        ],
        "provenance": {"signer": "ed25519:fixture", "signature": "fixture-signature"},
    }


def test_contract_manifest_keywords_match_normative_kotodama_grammar() -> None:
    root = Path(__file__).resolve().parents[3]
    grammar = (root / "crates" / "kotodama_lang" / "grammar" / "v1.lex").read_text(
        encoding="utf-8"
    )
    expected = {
        line.split("\t")[1]
        for line in grammar.splitlines()
        if line.startswith("keyword\t")
    }
    assert client_module._KOTODAMA_RESERVED_IDENTIFIERS == expected
    assert expected.isdisjoint({"contract", "entry", "init", "upgrade"})

    semantic = (root / "crates" / "kotodama_lang" / "src" / "semantic.rs").read_text(
        encoding="utf-8"
    )
    type_table = re.search(
        r"pub const V1_SOURCE_TYPE_NAMES: &\[&str\] = &\[(.*?)\];",
        semantic,
        re.DOTALL,
    )
    assert type_table is not None
    type_names = set(re.findall(r'"([A-Za-z_][A-Za-z0-9_]*)"', type_table.group(1)))
    assert client_module._KOTODAMA_RESERVED_DECLARATION_IDENTIFIERS == type_names | {
        "AxtDescriptor",
        "AssetHandle",
        "ProofBlob",
        "SoracloudRequest",
        "SoracloudResponse",
        "state_map_get",
    }


def test_contract_manifest_preserves_exact_v1_interface_shape() -> None:
    payload = _full_manifest_payload()
    manifest = ContractManifest.from_payload(payload)

    assert manifest.seiyaku_name == "Ledger"
    assert manifest.code_hash == "b" * 64
    assert manifest.abi_hash == "d" * 64
    assert manifest.access_set_hints is not None
    assert manifest.access_set_hints.dynamic_reads[0].max_keys == 64
    assert manifest.entrypoints is not None
    entrypoint = manifest.entrypoints[0]
    assert entrypoint.kind is ContractEntrypointKind.KOTOAGE
    assert entrypoint.argument_schema is not None
    assert entrypoint.argument_schema.fields[0].type.word_count == 2
    assert entrypoint.argument_schema.fields[1].type.word_count == 1
    assert entrypoint.argument_schema.fields[1].type.nodes[0].kind is (
        EntrypointValueTypeNodeKindV1.LIST
    )
    assert entrypoint.argument_schema.fields[1].type.canonical_type_name == "List<Name, 64>"
    assert entrypoint.return_schema is not None
    assert entrypoint.return_schema.word_count == 1
    assert entrypoint.return_schema.nodes[2].value is EntrypointValueKindV1.BOOL
    assert entrypoint.triggers[0]["callback"]["entrypoint"] == "transfer"
    assert entrypoint.triggers[0]["metadata"]["round"] == 7
    assert manifest.states is not None
    assert manifest.states[0].type_name == "StateMap<AccountId, Amount>"
    assert manifest.error_codes is not None
    assert manifest.error_codes[0].code == 1001
    assert manifest.kotoba is not None
    assert manifest.kotoba[0].translations[1].language == "ja"
    assert manifest.provenance is not None
    assert manifest.provenance["signer"] == "ed25519:fixture"

    payload["entrypoints"][0]["triggers"][0]["metadata"]["round"] = 99
    payload["provenance"]["signer"] = "mutated"
    assert entrypoint.triggers[0]["metadata"]["round"] == 7
    assert manifest.provenance["signer"] == "ed25519:fixture"


@pytest.mark.parametrize("view_name", tuple(_QUERY_VIEW_LAYOUTS))
def test_entrypoint_query_page_uses_each_exact_reserved_flat_schema(
    view_name: str,
) -> None:
    page = _query_page_payload(view_name)
    schema = EntrypointValueTypeV1.from_payload(page)
    assert schema.word_count == 2
    assert schema.canonical_type_name == f"QueryPage<{view_name}>"

    for mutate in (
        lambda value: value["nodes"][1]["value"].__setitem__("capacity", 32),
        lambda value: value["nodes"][2]["value"].__setitem__("name", "UnknownView"),
        lambda value: value["nodes"][-1]["value"].__setitem__("kind", "String"),
    ):
        forged = deepcopy(page)
        mutate(forged)
        with pytest.raises(TypeError, match="forged reserved"):
            EntrypointValueTypeV1.from_payload(forged)


@pytest.mark.parametrize("view_name", tuple(_QUERY_VIEW_LAYOUTS))
def test_entrypoint_rejects_each_forged_reserved_view(view_name: str) -> None:
    canonical = {"nodes": _query_view_nodes(view_name)}
    schema = EntrypointValueTypeV1.from_payload(canonical)
    assert schema.canonical_type_name == view_name

    wrong_fields = deepcopy(canonical)
    wrong_fields["nodes"][0]["value"]["fields"][0] = "forged"
    with pytest.raises(TypeError, match="forged reserved"):
        EntrypointValueTypeV1.from_payload(wrong_fields)

    wrong_leaf = deepcopy(canonical)
    wrong_leaf["nodes"][1]["value"]["kind"] = "Bool"
    with pytest.raises(TypeError, match="forged reserved"):
        EntrypointValueTypeV1.from_payload(wrong_leaf)


def test_entrypoint_ordinary_struct_keeps_its_nominal_struct_prefix() -> None:
    pair = {
        "nodes": [
            {
                "kind": "Struct",
                "value": {"name": "Pair", "fields": ["left", "right"]},
            },
            _leaf_node("Int"),
            _leaf_node("Bool"),
        ]
    }

    schema = EntrypointValueTypeV1.from_payload(pair)

    assert schema.canonical_type_name == "struct Pair"


def test_entrypoint_flat_list_schema_enforces_the_exact_depth_boundary() -> None:
    at_limit = {
        "nodes": [
            *(
                {"kind": "List", "value": {"capacity": 1}}
                for _ in range(255)
            ),
            {"kind": "Leaf", "value": {"kind": "Int", "value": None}},
        ]
    }
    schema = EntrypointValueTypeV1.from_payload(at_limit)
    assert schema.word_count == 1
    assert schema.canonical_type_name.count("List<") == 255

    for malformed in (
        {"nodes": []},
        {"nodes": [{"kind": "List", "value": {"capacity": 1}}]},
        {
            "nodes": [
                _leaf_node("Int"),
                _leaf_node("Bool"),
            ]
        },
        {
            "nodes": [
                {"kind": "List", "value": {"capacity": 1, "element": {}}},
                {"kind": "Leaf", "value": {"kind": "Int", "value": None}},
            ]
        },
        {
            "nodes": [
                {"kind": "List", "value": {"capacity": 0}},
                _leaf_node("Int"),
            ]
        },
        {
            "nodes": [
                {"kind": "List", "value": {"capacity": 65}},
                _leaf_node("Int"),
            ]
        },
        {
            "nodes": [
                *(
                    {"kind": "List", "value": {"capacity": 1}}
                    for _ in range(256)
                ),
                {"kind": "Leaf", "value": {"kind": "Int", "value": None}},
            ]
        },
    ):
        with pytest.raises(TypeError):
            EntrypointValueTypeV1.from_payload(malformed)

@pytest.mark.parametrize(
    ("filename", "expected_name"),
    [
        ("authority_probe.manifest.json", "AuthorityProbe"),
        ("irohaswap.manifest.json", "IrohaSwap"),
        ("ivm_smoke.manifest.json", "SmokeTransfer"),
        ("prediction_market.manifest.json", "PredictionMarket"),
    ],
)
def test_contract_manifest_decodes_checked_in_canonical_kotodama_manifests(
    filename: str, expected_name: str
) -> None:
    root = Path(__file__).resolve().parents[3]
    payload = json.loads((root / "demo" / filename).read_text(encoding="utf-8"))

    manifest = ContractManifest.from_payload(payload)

    assert manifest.seiyaku_name == expected_name
    assert manifest.code_hash is not None and len(manifest.code_hash) == 64
    assert manifest.abi_hash is not None and len(manifest.abi_hash) == 64
    assert manifest.entrypoints


@pytest.mark.parametrize(
    "value",
    [
        "",
        " Ledger ",
        "seiyaku",
        "match",
        "i64",
        "state_map_get",
        "__kotodama_link_forged",
        "始まり",
        7,
        [],
        {},
    ],
)
def test_contract_manifest_rejects_invalid_seiyaku_name(value: object) -> None:
    with pytest.raises(TypeError, match="seiyaku_name"):
        ContractManifest.from_payload({"seiyaku_name": value})


@pytest.mark.parametrize(
    "value",
    [
        "b" * 64,
        "hash:" + "b" * 64 + "#ABA2",
        "hash:" + "B" * 64 + "#0000",
        "hash:" + "B" * 64 + "#aba2",
        "hash:" + "A" * 64 + "#0E5B",
        "blake2b32:" + "B" * 64 + "#ABA2",
    ],
)
def test_contract_manifest_rejects_noncanonical_hash_literals(value: str) -> None:
    with pytest.raises(TypeError, match="Hash|checksum|marker"):
        ContractManifest.from_payload({"code_hash": value})


def test_contract_manifest_rejects_non_object_provenance() -> None:
    with pytest.raises(TypeError, match="provenance"):
        ContractManifest.from_payload({"provenance": "not-an-object"})


def test_contract_manifest_record_cross_checks_hash_conveniences() -> None:
    manifest_payload = _full_manifest_payload()
    record = ContractManifestRecord.from_payload(
        {
            "manifest": manifest_payload,
            "code_hash": "b" * 64,
            "abi_hash": "d" * 64,
        }
    )

    assert record.code_hash == record.manifest.code_hash == "b" * 64
    assert record.abi_hash == record.manifest.abi_hash == "d" * 64


@pytest.mark.parametrize(
    "mutation",
    [
        {"code_hash": "d" * 64, "abi_hash": "d" * 64},
        {"code_hash": "B" * 64, "abi_hash": "d" * 64},
        {"abi_hash": "d" * 64},
        {"code_hash": "b" * 64, "abi_hash": "d" * 64, "code_bytes": None},
    ],
)
def test_contract_manifest_record_rejects_mismatched_or_noncanonical_hashes(
    mutation: Dict[str, Any]
) -> None:
    payload: Dict[str, Any] = {"manifest": _full_manifest_payload()}
    payload.update(mutation)

    with pytest.raises(TypeError, match="hash|code_bytes"):
        ContractManifestRecord.from_payload(payload)


@pytest.mark.parametrize("retired", ["Public", "Init", "Upgrade", "entry"])
def test_contract_manifest_rejects_noncanonical_entrypoint_kinds(retired: str) -> None:
    payload = _full_manifest_payload()
    payload["entrypoints"][0]["kind"] = {"kind": retired, "value": None}

    with pytest.raises(TypeError, match="entrypoint kind"):
        ContractManifest.from_payload(payload)


@pytest.mark.parametrize(
    ("label", "expected", "name"),
    [
        ("Kotoage", ContractEntrypointKind.KOTOAGE, "transfer"),
        ("View", ContractEntrypointKind.VIEW, "transfer"),
        ("Hajimari", ContractEntrypointKind.HAJIMARI, "始まり"),
        ("Kaizen", ContractEntrypointKind.KAIZEN, "kaizen"),
    ],
)
def test_contract_manifest_accepts_only_branded_v1_entrypoint_kinds(
    label: str, expected: ContractEntrypointKind, name: str
) -> None:
    payload = _full_manifest_payload()
    payload["entrypoints"][0]["kind"] = {"kind": label, "value": None}
    payload["entrypoints"][0]["name"] = name
    if expected is not ContractEntrypointKind.KOTOAGE:
        payload["entrypoints"][0]["triggers"] = []
    if expected in {ContractEntrypointKind.HAJIMARI, ContractEntrypointKind.KAIZEN}:
        payload["entrypoints"][0]["permission"] = None

    manifest = ContractManifest.from_payload(payload)

    assert manifest.entrypoints is not None
    assert manifest.entrypoints[0].kind is expected


@pytest.mark.parametrize(
    "mutate",
    [
        lambda entrypoint: entrypoint.update(
            {"name": "init", "kind": {"kind": "Hajimari", "value": None}}
        ),
        lambda entrypoint: entrypoint.update(
            {"name": "hajimari", "kind": {"kind": "Kotoage", "value": None}}
        ),
        lambda entrypoint: entrypoint.update({"permission": None}),
        lambda entrypoint: entrypoint.update(
            {
                "name": "kaizen",
                "kind": {"kind": "Kaizen", "value": None},
                "permission": "Upgrade",
            }
        ),
    ],
)
def test_contract_manifest_rejects_lifecycle_or_authorization_forgery(
    mutate: Any,
) -> None:
    payload = _full_manifest_payload()
    mutate(payload["entrypoints"][0])

    with pytest.raises(TypeError, match="canonical exact V1 interface"):
        ContractManifest.from_payload(payload)


def test_contract_manifest_rejects_retired_or_ambiguous_manifest_fields() -> None:
    for retired in ("contract_name", "contractName"):
        payload = _full_manifest_payload()
        payload[retired] = "Legacy"
        with pytest.raises(TypeError, match="unsupported fields"):
            ContractManifest.from_payload(payload)


def test_contract_manifest_rejects_duplicate_and_unsafe_trigger_metadata() -> None:
    payload = _full_manifest_payload()
    payload["entrypoints"].append(deepcopy(payload["entrypoints"][0]))
    with pytest.raises(TypeError, match="duplicate entrypoint"):
        ContractManifest.from_payload(payload)

    payload = _full_manifest_payload()
    payload["entrypoints"].insert(
        0,
        {
            "name": "read",
            "kind": {"kind": "View", "value": None},
            "params": [],
            "argument_schema": None,
            "return_type": None,
            "return_schema": None,
            "permission": None,
            "read_keys": [],
            "write_keys": [],
            "access_hints_complete": True,
            "access_hints_skipped": [],
            "triggers": [],
        },
    )
    payload["entrypoints"][1]["triggers"][0]["callback"]["entrypoint"] = "read"
    with pytest.raises(TypeError, match="must target kotoage/言挙げ"):
        ContractManifest.from_payload(payload)

    payload = _full_manifest_payload()
    payload["entrypoints"][0]["triggers"][0]["filter"] += "\n"
    with pytest.raises(TypeError, match="standard-base64"):
        ContractManifest.from_payload(payload)


def test_contract_manifest_rejects_non_null_enum_payloads() -> None:
    payload = _full_manifest_payload()
    payload["entrypoints"][0]["kind"]["value"] = "Kotoage"

    with pytest.raises(TypeError, match="must be null"):
        ContractManifest.from_payload(payload)


@pytest.mark.parametrize(
    ("mutate", "message"),
    [
        (
            lambda payload: payload["entrypoints"][0]["argument_schema"]["fields"][1][
                "ty"
            ]["nodes"][0]["value"].__setitem__("capacity", 65),
            "capacity",
        ),
        (
            lambda payload: payload["entrypoints"][0]["argument_schema"]["fields"][1][
                "ty"
            ]["nodes"][0]["value"].__setitem__(
                "element",
                {
                    "nodes": [
                        {"kind": "Leaf", "value": {"kind": "Name", "value": None}}
                    ]
                },
            ),
            "only `capacity`",
        ),
        (
            lambda payload: payload["entrypoints"][0]["argument_schema"]["fields"][1][
                "ty"
            ]["nodes"].pop(),
            "canonical V1 schema",
        ),
        (
            lambda payload: payload["entrypoints"][0]["return_schema"]["nodes"].append(
                {"kind": "Leaf", "value": {"kind": "Bool", "value": None}}
            ),
            "canonical V1 schema",
        ),
        (
            lambda payload: payload["entrypoints"][0]["argument_schema"]["fields"][0][
                "ty"
            ]["nodes"][0]["value"].__setitem__("fields", ["memo", "memo"]),
            "canonical V1 schema",
        ),
        (
            lambda payload: payload["error_codes"][0].__setitem__("code", 0),
            "non-zero u32",
        ),
        (
            lambda payload: payload["entrypoints"][0]["params"][0].__setitem__(
                "name", "different"
            ),
            "canonical exact V1 interface",
        ),
        (
            lambda payload: payload["entrypoints"][0].__setitem__("return_type", None),
            "canonical exact V1 interface",
        ),
    ],
)
def test_contract_manifest_rejects_invalid_v1_shapes(mutate: Any, message: str) -> None:
    payload = deepcopy(_full_manifest_payload())
    mutate(payload)

    with pytest.raises(TypeError, match=message):
        ContractManifest.from_payload(payload)


def test_contract_manifest_rejects_overwide_argument_record() -> None:
    payload = _full_manifest_payload()
    payload["entrypoints"][0]["argument_schema"]["fields"] = [
        {
            "name": f"field_{index}",
            "ty": {
                "nodes": [
                    {"kind": "Leaf", "value": {"kind": "Bool", "value": None}}
                ]
            },
        }
        for index in range(14)
    ]

    with pytest.raises(TypeError, match="canonical V1 bounds"):
        ContractManifest.from_payload(payload)


def test_contract_manifest_rejects_overwide_return_record() -> None:
    payload = _full_manifest_payload()
    payload["entrypoints"][0]["return_type"] = "wide tuple"
    payload["entrypoints"][0]["return_schema"] = {
        "nodes": [
            {"kind": "Tuple", "value": 14},
            *[
                {"kind": "Leaf", "value": {"kind": "Bool", "value": None}}
                for _ in range(14)
            ],
        ]
    }

    with pytest.raises(TypeError, match="canonical exact V1 interface"):
        ContractManifest.from_payload(payload)
