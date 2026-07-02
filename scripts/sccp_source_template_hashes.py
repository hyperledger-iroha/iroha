#!/usr/bin/env python3
"""Shared SCCP active source-template hash denylist."""

from __future__ import annotations

from collections.abc import Mapping


SCCP_ACTIVE_SOURCE_TEMPLATE_FIELDS_BY_LANE: dict[str, tuple[str, ...]] = {
    "ETH": (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "finality_policy_hash",
    ),
    "BSC": (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "finality_policy_hash",
    ),
    "Solana": (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "source_state_verifier_hash",
        "finality_policy_hash",
    ),
    "TON": (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "source_state_verifier_hash",
        "finality_policy_hash",
    ),
    "TRON": (
        "source_trust_anchor_hash",
        "consensus_verifier_hash",
        "message_inclusion_verifier_hash",
        "finality_policy_hash",
    ),
}


SCCP_ACTIVE_SOURCE_TEMPLATE_COMPONENT_HASHES: tuple[tuple[str, str, bytes], ...] = (
    (
        "ETH",
        "source_trust_anchor_hash",
        bytes.fromhex("6c1a909de9cdc3cc7235a6d825f1505c479bec419cab34df6d512525ac5be635"),
    ),
    (
        "ETH",
        "consensus_verifier_hash",
        bytes.fromhex("cfd7d00fb88cd7a63a302b14d04f6556fa2f1802e1dc55b248d0bcc2e87f2d78"),
    ),
    (
        "ETH",
        "message_inclusion_verifier_hash",
        bytes.fromhex("77c4118807548732f734bb2d54c500b20f17f70d2c175e08689d3353137212db"),
    ),
    (
        "ETH",
        "finality_policy_hash",
        bytes.fromhex("14669200e203a45030eb933e92b482d779f3a5a7a3acfce8b2d08728e1aec959"),
    ),
    (
        "BSC",
        "source_trust_anchor_hash",
        bytes.fromhex("8fb2248473e64b3f9c6ebdd7cda50fc9e4a01ed2bc69463d9efb4ca842c28ef7"),
    ),
    (
        "BSC",
        "consensus_verifier_hash",
        bytes.fromhex("4e945a4bcdc7e04e17c85d7371b19dd9f0fbe2c0926c08075f1a1a18e52a294d"),
    ),
    (
        "BSC",
        "message_inclusion_verifier_hash",
        bytes.fromhex("abdbf2df079f7710cb6e3cf3fa27bad638af0103a43b3603ab78988b0de13fde"),
    ),
    (
        "BSC",
        "finality_policy_hash",
        bytes.fromhex("754f806689978836e4fa662f109497a29902551aa9dfa2fd0e26ea9d72d32178"),
    ),
    (
        "Solana",
        "source_trust_anchor_hash",
        bytes.fromhex("113bdb7601d84f2098daec386346a7123857d181b3ac5bd23df50fa9e1b2cbe3"),
    ),
    (
        "Solana",
        "consensus_verifier_hash",
        bytes.fromhex("97ea89019e6c79305d06dfc27640ee14a6b42ba6eaf86e1835ee9b433dba48ba"),
    ),
    (
        "Solana",
        "message_inclusion_verifier_hash",
        bytes.fromhex("b8358bfef1e428a6a7e9115687cb2b88d9c21dad4021bea3e11d43489eb3dcb0"),
    ),
    (
        "Solana",
        "source_state_verifier_hash",
        bytes.fromhex("6b4e4106bbb6b343ae1a4a36c9c68756d4454d2167c9b8b2ee3225e39fb0a48b"),
    ),
    (
        "Solana",
        "finality_policy_hash",
        bytes.fromhex("9df7ea90cf1bbba036788b14804f63f4be1e908390be89524fd4486f74344f56"),
    ),
    (
        "TON",
        "source_trust_anchor_hash",
        bytes.fromhex("d83b3a3eb920ac8338533535cf0d6c69c69d507e84aef8ec2094564b8427c56c"),
    ),
    (
        "TON",
        "consensus_verifier_hash",
        bytes.fromhex("b0225e16477ea3420f7d0de76b87b6e99a43ab97f445d8565a384d4b655bc473"),
    ),
    (
        "TON",
        "message_inclusion_verifier_hash",
        bytes.fromhex("89254256421c15da8c92842c7d6f448ef6c1d5ca1e2a173754643425fcee6353"),
    ),
    (
        "TON",
        "source_state_verifier_hash",
        bytes.fromhex("540205f876591604ccf39f72a051ac5e82647c9e48dbd48cb129d2543971a34f"),
    ),
    (
        "TON",
        "finality_policy_hash",
        bytes.fromhex("50044ee6db0eb0cdef097e69406b6c30d3406d8f784e8ba34e9b923b38bd0c43"),
    ),
    (
        "TRON",
        "source_trust_anchor_hash",
        bytes.fromhex("3550934cbdfe49449ec4aa383dcea7674541fedf66ab6159b1ed2f2c0be4755c"),
    ),
    (
        "TRON",
        "consensus_verifier_hash",
        bytes.fromhex("8a1de96a869b2f28f197a7835597f17cf77ff45f7cbb77da2f7c48e87df8c5ea"),
    ),
    (
        "TRON",
        "message_inclusion_verifier_hash",
        bytes.fromhex("4cad5d62d2be7ad0e4f91de26940417dc18c4d9112256ce9d5a6e2188fbedbd6"),
    ),
    (
        "TRON",
        "finality_policy_hash",
        bytes.fromhex("ad5a6a4f200e070400b5aaa1b7976c639e67571eb711eb6f69d01e3615423864"),
    ),
)


def sccp_active_source_template_component_hash_errors(
    entries: tuple[object, ...] = SCCP_ACTIVE_SOURCE_TEMPLATE_COMPONENT_HASHES,
) -> tuple[str, ...]:
    """Return bounded schema errors for active SCCP source-template hashes."""

    errors: list[str] = []
    seen_fields: set[tuple[str, str]] = set()
    seen_hashes: dict[bytes, tuple[str, str]] = {}
    fields_by_lane: dict[str, list[str]] = {
        lane: [] for lane in SCCP_ACTIVE_SOURCE_TEMPLATE_FIELDS_BY_LANE
    }

    for index, entry in enumerate(entries):
        if not isinstance(entry, tuple) or len(entry) != 3:
            errors.append(f"entry {index} must be a lane, field, hash tuple")
            continue

        lane, field, template_hash = entry
        if (
            not isinstance(lane, str)
            or lane not in SCCP_ACTIVE_SOURCE_TEMPLATE_FIELDS_BY_LANE
        ):
            errors.append(f"entry {index} lane must be an active launch lane")
            continue
        expected_fields = SCCP_ACTIVE_SOURCE_TEMPLATE_FIELDS_BY_LANE[lane]

        if not isinstance(field, str) or field not in expected_fields:
            errors.append(f"entry {index} field must be expected for lane {lane}")
            continue

        lane_field = (lane, field)
        fields_by_lane[lane].append(field)
        if lane_field in seen_fields:
            errors.append(f"entry {index} duplicates lane field {lane}.{field}")
        else:
            seen_fields.add(lane_field)

        if (
            not isinstance(template_hash, bytes)
            or len(template_hash) != 32
            or not any(template_hash)
        ):
            errors.append(f"entry {index} hash must be non-zero bytes32")
            continue

        previous = seen_hashes.get(template_hash)
        if previous is not None:
            previous_lane, previous_field = previous
            errors.append(
                f"entry {index} hash duplicates {previous_lane}.{previous_field}"
            )
        else:
            seen_hashes[template_hash] = lane_field

    for lane, expected_fields in SCCP_ACTIVE_SOURCE_TEMPLATE_FIELDS_BY_LANE.items():
        if tuple(fields_by_lane[lane]) != expected_fields:
            errors.append(f"lane {lane} fields must match active launch template order")

    return tuple(errors)


def _validate_sccp_active_source_template_component_hashes() -> None:
    """Fail closed when the checked-in active SCCP source-template table drifts."""

    errors = sccp_active_source_template_component_hash_errors()
    if errors:
        raise RuntimeError(
            "invalid SCCP active source-template denylist: " + "; ".join(errors)
        )


_validate_sccp_active_source_template_component_hashes()


def sccp_active_source_template_component_hashes() -> tuple[tuple[str, str, bytes], ...]:
    """Return built-in source-template hashes for active SCCP launch lanes."""

    return SCCP_ACTIVE_SOURCE_TEMPLATE_COMPONENT_HASHES


def sccp_source_template_hash_match(
    value: bytes,
    *,
    local_template_hashes: Mapping[str, bytes] | None = None,
) -> tuple[str | None, str] | None:
    """Return the local or active-lane template hash matched by ``value``."""

    if local_template_hashes is not None:
        for field, template_hash in local_template_hashes.items():
            if value == template_hash:
                return (None, field)

    for lane, field, template_hash in SCCP_ACTIVE_SOURCE_TEMPLATE_COMPONENT_HASHES:
        if value == template_hash:
            return (lane, field)

    return None


def sccp_source_template_hash_human_label(match: tuple[str | None, str]) -> str:
    """Return a diagnostic label for a template-hash match."""

    lane, field = match
    label = field.replace("_", " ")
    if lane is None:
        return label
    return f"{lane} {label}"
