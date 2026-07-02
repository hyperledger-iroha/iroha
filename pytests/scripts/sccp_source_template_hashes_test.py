from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


SCRIPTS_DIR = Path(__file__).resolve().parents[2] / "scripts"


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
    assert all(isinstance(template_hash, bytes) for template_hash in template_hashes)
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
