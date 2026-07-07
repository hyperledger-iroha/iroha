from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"


class HostileTemplateString(str):
    def __hash__(self):
        raise AssertionError("secret-token source template string was hashed")

    def __eq__(self, other):
        raise AssertionError("secret-token source template string was compared")

    def __str__(self):
        raise AssertionError("secret-token source template string was rendered")

    def replace(self, *args, **kwargs):
        raise AssertionError("secret-token source template string was replaced")


class HostileTemplateBytes(bytes):
    def __hash__(self):
        raise AssertionError("secret-token source template hash was hashed")

    def __eq__(self, other):
        raise AssertionError("secret-token source template hash was compared")

    def __len__(self):
        raise AssertionError("secret-token source template hash length was read")

    def __iter__(self):
        raise AssertionError("secret-token source template hash was iterated")


class HostileTemplateTuple(tuple):
    def __new__(cls, values):
        return tuple.__new__(cls, values)

    def __len__(self):
        raise AssertionError("secret-token source template tuple length was read")

    def __iter__(self):
        raise AssertionError("secret-token source template tuple was iterated")

    def __getitem__(self, index):
        raise AssertionError("secret-token source template tuple was indexed")


class HostileTemplateHashProbe:
    def __eq__(self, other):
        raise AssertionError("secret-token source template hash probe was compared")

    def __len__(self):
        raise AssertionError("secret-token source template hash probe length was read")

    def __iter__(self):
        raise AssertionError("secret-token source template hash probe was iterated")


class HostileTemplateMapping:
    def __init__(self, field, template_hash):
        self.field = field
        self.template_hash = template_hash

    def items(self):
        return ((self.field, self.template_hash),)


def load_script(name):
    script_path = SCRIPTS_DIR / f"{name}.py"
    spec = spec_from_file_location(name, script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def active_lane_hashes(helper, lane):
    return {
        field: template_hash
        for template_lane, field, template_hash in (
            helper.sccp_active_source_template_component_hashes()
        )
        if template_lane == lane
    }


def test_active_source_template_denylist_is_canonical_and_unique():
    helper = load_script("sccp_source_template_hashes")
    entries = helper.sccp_active_source_template_component_hashes()
    expected_pairs = (
        ("ETH", "source_trust_anchor_hash"),
        ("ETH", "consensus_verifier_hash"),
        ("ETH", "message_inclusion_verifier_hash"),
        ("ETH", "finality_policy_hash"),
        ("BSC", "source_trust_anchor_hash"),
        ("BSC", "consensus_verifier_hash"),
        ("BSC", "message_inclusion_verifier_hash"),
        ("BSC", "finality_policy_hash"),
        ("Solana", "source_trust_anchor_hash"),
        ("Solana", "consensus_verifier_hash"),
        ("Solana", "message_inclusion_verifier_hash"),
        ("Solana", "source_state_verifier_hash"),
        ("Solana", "finality_policy_hash"),
        ("TON", "source_trust_anchor_hash"),
        ("TON", "consensus_verifier_hash"),
        ("TON", "message_inclusion_verifier_hash"),
        ("TON", "source_state_verifier_hash"),
        ("TON", "finality_policy_hash"),
        ("TRON", "source_trust_anchor_hash"),
        ("TRON", "consensus_verifier_hash"),
        ("TRON", "message_inclusion_verifier_hash"),
        ("TRON", "finality_policy_hash"),
    )

    assert tuple((lane, field) for lane, field, _hash in entries) == expected_pairs
    assert helper.sccp_active_source_template_component_hash_errors(entries) == ()
    template_hashes = [template_hash for _lane, _field, template_hash in entries]
    assert all(type(template_hash) is bytes for template_hash in template_hashes)
    assert all(len(template_hash) == 32 for template_hash in template_hashes)
    assert all(any(template_hash) for template_hash in template_hashes)
    assert len(template_hashes) == len(set(template_hashes))


def test_active_source_template_denylist_validator_rejects_bad_entries():
    helper = load_script("sccp_source_template_hashes")
    entries = list(helper.sccp_active_source_template_component_hashes())

    zero_hash_entries = entries.copy()
    lane, field, _hash = zero_hash_entries[0]
    zero_hash_entries[0] = (lane, field, bytes(32))
    assert (
        "entry 0 hash must be non-zero bytes32"
        in helper.sccp_active_source_template_component_hash_errors(
            tuple(zero_hash_entries)
        )
    )

    duplicate_hash_entries = entries.copy()
    lane, field, _hash = duplicate_hash_entries[1]
    duplicate_hash_entries[1] = (lane, field, entries[0][2])
    assert (
        "entry 1 hash duplicates ETH.source_trust_anchor_hash"
        in helper.sccp_active_source_template_component_hash_errors(
            tuple(duplicate_hash_entries)
        )
    )

    duplicate_field_entries = tuple(entries + [entries[0]])
    duplicate_field_errors = (
        helper.sccp_active_source_template_component_hash_errors(
            duplicate_field_entries
        )
    )
    assert "entry 22 duplicates lane field ETH.source_trust_anchor_hash" in (
        duplicate_field_errors
    )
    assert "lane ETH fields must match active launch template order" in (
        duplicate_field_errors
    )

    malformed_entry_errors = helper.sccp_active_source_template_component_hash_errors(
        tuple([*entries[:-1], ("unexpected", "source_trust_anchor_hash", b"\x01" * 32)])
    )
    assert "entry 21 lane must be an active launch lane" in malformed_entry_errors
    assert "lane TRON fields must match active launch template order" in (
        malformed_entry_errors
    )

    missing_lane_errors = helper.sccp_active_source_template_component_hash_errors(
        tuple(entries[:-4])
    )
    assert "lane TRON fields must match active launch template order" in (
        missing_lane_errors
    )


def test_source_template_hashes_reject_string_subclasses_without_hooks():
    helper = load_script("sccp_source_template_hashes")
    hostile_lane = HostileTemplateString("ETH")
    hostile_field = HostileTemplateString("source_trust_anchor_hash")
    template_hash = b"\x01" * 32

    lane_errors = helper.sccp_active_source_template_component_hash_errors(
        ((hostile_lane, "source_trust_anchor_hash", template_hash),)
    )
    assert "entry 0 lane must be an active launch lane" in lane_errors
    assert "secret-token" not in "; ".join(lane_errors)

    field_errors = helper.sccp_active_source_template_component_hash_errors(
        (("ETH", hostile_field, template_hash),)
    )
    assert "entry 0 field must be expected for lane ETH" in field_errors
    assert "secret-token" not in "; ".join(field_errors)

    try:
        helper.sccp_source_template_hash_match(
            template_hash,
            local_template_hashes=HostileTemplateMapping(
                hostile_field,
                template_hash,
            ),
        )
    except TypeError as exc:
        rendered = str(exc)
        assert rendered == "source template local field must be a builtin string"
        assert "secret-token" not in rendered
    else:
        raise AssertionError("source template match accepted a hostile local field")

    human_label_cases = (
        (
            (hostile_lane, "source_trust_anchor_hash"),
            "source template match lane must be a builtin string",
        ),
        (
            (None, hostile_field),
            "source template match field must be a builtin string",
        ),
    )
    for match, expected_message in human_label_cases:
        try:
            helper.sccp_source_template_hash_human_label(match)
        except TypeError as exc:
            rendered = str(exc)
            assert rendered == expected_message
            assert "secret-token" not in rendered
        else:
            raise AssertionError("source template human label accepted hostile text")


def test_source_template_hashes_reject_bytes_subclasses_without_hooks():
    helper = load_script("sccp_source_template_hashes")
    entries = helper.sccp_active_source_template_component_hashes()
    lane, field, _template_hash = entries[0]
    builtin_hash = b"\x01" * 32
    hostile_hash = HostileTemplateBytes(builtin_hash)

    denylist_errors = helper.sccp_active_source_template_component_hash_errors(
        ((lane, field, hostile_hash),)
    )
    assert "entry 0 hash must be non-zero bytes32" in denylist_errors
    assert "secret-token" not in "; ".join(denylist_errors)

    assert (
        helper.sccp_source_template_hash_match(
            hostile_hash,
            local_template_hashes={"local_anchor_hash": builtin_hash},
        )
        is None
    )

    try:
        helper.sccp_source_template_hash_match(
            builtin_hash,
            local_template_hashes={"local_anchor_hash": hostile_hash},
        )
    except TypeError as exc:
        rendered = str(exc)
        assert rendered == "source template local hash must be builtin non-zero bytes32"
        assert "secret-token" not in rendered
    else:
        raise AssertionError("source template match accepted a hostile local hash")

    assert (
        helper.sccp_source_template_hash_match(
            HostileTemplateHashProbe(),
            local_template_hashes={"local_anchor_hash": builtin_hash},
        )
        is None
    )


def test_source_template_hashes_reject_tuple_subclasses_without_hooks():
    helper = load_script("sccp_source_template_hashes")
    hostile_entry = HostileTemplateTuple(("ETH", "source_trust_anchor_hash", b"\x01" * 32))
    hostile_match = HostileTemplateTuple(("ETH", "source_trust_anchor_hash"))

    errors = helper.sccp_active_source_template_component_hash_errors((hostile_entry,))
    assert "entry 0 must be a lane, field, hash tuple" in errors
    assert "secret-token" not in "; ".join(errors)

    try:
        helper.sccp_source_template_hash_human_label(hostile_match)
    except TypeError as exc:
        rendered = str(exc)
        assert rendered == "source template match must be a lane, field tuple"
        assert "secret-token" not in rendered
    else:
        raise AssertionError("source template human label accepted hostile tuple")


def test_source_template_hash_match_labels_local_and_foreign_templates():
    helper = load_script("sccp_source_template_hashes")
    entries = helper.sccp_active_source_template_component_hashes()
    eth_trust_anchor = entries[0][2]
    solana_consensus = next(
        template_hash
        for lane, field, template_hash in entries
        if lane == "Solana" and field == "consensus_verifier_hash"
    )

    assert helper.sccp_source_template_hash_match(
        eth_trust_anchor,
        local_template_hashes={"local_anchor_hash": eth_trust_anchor},
    ) == (None, "local_anchor_hash")
    foreign_match = helper.sccp_source_template_hash_match(solana_consensus)
    assert foreign_match == ("Solana", "consensus_verifier_hash")
    assert (
        helper.sccp_source_template_hash_human_label(foreign_match)
        == "Solana consensus verifier hash"
    )


def test_active_source_template_denylist_matches_evidence_script_templates():
    helper = load_script("sccp_source_template_hashes")
    eth = load_script("sccp_eth_source_bridge_evidence")
    bsc = load_script("sccp_bsc_source_bridge_evidence")
    solana = load_script("sccp_solana_source_state_evidence")
    ton = load_script("sccp_ton_source_state_evidence")
    tron = load_script("sccp_tron_source_bridge_evidence")

    assert active_lane_hashes(helper, "ETH") == eth._template_component_hashes()
    assert active_lane_hashes(helper, "BSC") == bsc._template_component_hashes("mainnet")
    assert active_lane_hashes(helper, "Solana") == solana._template_component_hashes()
    assert active_lane_hashes(helper, "TON") == ton._template_component_hashes()
    assert active_lane_hashes(helper, "TRON") == tron._template_component_hashes()
