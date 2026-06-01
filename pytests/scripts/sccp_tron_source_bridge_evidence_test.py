import json
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path
from types import SimpleNamespace


TRON_DESTINATION_BINDING_VECTOR = (
    "17c953ad5b8c9a2b6f7102aca993fa7c427d018505cf4f58fac35ea454caba7f"
)
TRON_DESTINATION_BINDING_KEY_VECTOR = (
    "tron:0:5:"
    + "33" * 32
    + ":TJRabPrwbZy45sbavfcjinPJC18kjpRTv8:0x"
    + "bb" * 32
    + ":0x"
    + "cc" * 32
)
TRON_SOURCE_CONFIG_VECTOR = (
    "e986dd67bfa2307b4e00cf46bde41a88003a55c5b7fea311fa106614b2252f9d"
)
TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR = (
    "0e12ad03def9d75887d4d6437e63539cef97c54db4769881eeda757a88826364"
)
TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR = (
    "68c20262e44676bd5f3c4ec428f063373147a1ca14c5885648a9c651b3bcd8d8"
)
TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR = (
    "94dbe28a2fb16e043b83639b6dea8ec62f53679599ef1dd220fd13c71c7bdcb8"
)
TRON_DPOS_SOURCE_GATE_HASH_VECTOR = (
    "776e8ebaf68ce872b0596330e4eb0c26bc6151ea23cb45dcd46316bb1f12bd28"
)
TRON_ROUTE_ALLOWLIST_HASH_VECTOR = (
    "fea8effb3cddfa458ea79a5a9af6f2d2c33a460b3a66d9305963908c2a3ea67a"
)
TRON_ROUTE_CANARY_EVIDENCE_HASH = (
    "e0a96ff7e8f523599fd60fffe8bb3b9fda9519126b7ba00c89c922b323b64e56"
)
TRON_ROUTE_CANARY_TRANSACTION_ID = "fa" * 32
TRON_ROUTE_CANARY_BLOCK_NUMBER = 234
TRON_ROUTE_CANARY_BLOCK_TIMESTAMP = 567000
TRON_ROUTE_CANARY_MESSAGE_ID = "dd" * 32
TRON_ROUTE_CANARY_CALL_DATA_SHA256 = (
    "f96dfb36d47a61e7e80df4f19e00b78c12f9a3f3c542e8dac06a7422e1d5f951"
)
TRON_ROUTE_CANARY_PAYLOAD_HASH = "ab" * 32
TRON_ROUTE_CANARY_TARGET_DOMAIN = 5
TRON_ROUTE_CANARY_STATEMENT_HASH = "f1" * 32
TRON_ROUTE_CANARY_COMMITMENT_ROOT = "ee" * 32
TRON_ROUTE_CANARY_FINALITY_HEIGHT = "00" * 31 + "7b"
TRON_ROUTE_CANARY_FINALITY_BLOCK_HASH = "cd" * 32
TRON_ROUTE_CANARY_PROOF_VERSION = 1
TRON_ROUTE_CANARY_PROOF_SOURCE_DOMAIN = 0
TRON_ROUTE_CANARY_SIGNATURE_SHA256 = "c4" * 32
TRON_ROUTE_CANARY_SIGNATURE_RECOVERED_ADDRESS = (
    "41" + "7e5f4552091a69125d5dfcb7b8c2659029395bdf"
)
TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS = (
    TRON_ROUTE_CANARY_SIGNATURE_RECOVERED_ADDRESS
)
TRON_SOURCE_EVENT_DIGEST_VECTOR = "34" * 32
TRON_SOURCE_EVENT_CALL_DATA_VECTOR = (
    "06841e30"
    + "00" * 31
    + "05"
    + "00" * 32
    + TRON_SOURCE_EVENT_DIGEST_VECTOR
)
TRON_SOURCE_RUNTIME_BYTECODE = "6001600055"
TRON_DESTINATION_RUNTIME_BYTECODE = "6002600055"


def load_evidence_module():
    script_path = (
        Path(__file__).resolve().parents[2]
        / "scripts"
        / "sccp_tron_source_bridge_evidence.py"
    )
    spec = spec_from_file_location("sccp_tron_source_bridge_evidence", script_path)
    module = module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def sample_full_toml_args():
    return SimpleNamespace(
        source_domain=5,
        target_domain=0,
        bridge_address=bytes.fromhex("11" * 20),
        owner_address=bytes.fromhex("22" * 20),
        network_id=bytes.fromhex("33" * 32),
        expected_config_hash=bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR),
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=bytes.fromhex(
            TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        ),
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
        expected_source_verifier_material_hash=bytes.fromhex(
            TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        ),
        expected_source_adapter_engine_deployment_hash=bytes.fromhex(
            TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        ),
        expected_tron_dpos_source_gate_hash=bytes.fromhex(
            TRON_DPOS_SOURCE_GATE_HASH_VECTOR
        ),
        destination_verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        destination_verifier_code_hash=bytes.fromhex("bb" * 32),
        destination_verifier_key_hash=bytes.fromhex("cc" * 32),
        destination_source_domain=0,
        destination_target_domain=5,
        destination_proof_family="stark-fri-v1",
        expected_destination_binding_hash=bytes.fromhex(
            TRON_DESTINATION_BINDING_VECTOR
        ),
        route_allowlist_hash=bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR),
        route_canary_evidence_hash=bytes.fromhex(TRON_ROUTE_CANARY_EVIDENCE_HASH),
        route_canary_transaction_id=bytes.fromhex(TRON_ROUTE_CANARY_TRANSACTION_ID),
        route_canary_transaction_owner_address=bytes.fromhex(
            TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS
        ),
        route_canary_block_number=TRON_ROUTE_CANARY_BLOCK_NUMBER,
        route_canary_block_timestamp=TRON_ROUTE_CANARY_BLOCK_TIMESTAMP,
        route_canary_log_index=0,
        route_canary_message_id=bytes.fromhex(TRON_ROUTE_CANARY_MESSAGE_ID),
        route_canary_call_data_sha256=bytes.fromhex(
            TRON_ROUTE_CANARY_CALL_DATA_SHA256
        ),
        route_canary_payload_hash=bytes.fromhex(TRON_ROUTE_CANARY_PAYLOAD_HASH),
        route_canary_target_domain=TRON_ROUTE_CANARY_TARGET_DOMAIN,
        route_canary_statement_hash=bytes.fromhex(TRON_ROUTE_CANARY_STATEMENT_HASH),
        route_canary_commitment_root=bytes.fromhex(
            TRON_ROUTE_CANARY_COMMITMENT_ROOT
        ),
        route_canary_finality_height=bytes.fromhex(TRON_ROUTE_CANARY_FINALITY_HEIGHT),
        route_canary_finality_block_hash=bytes.fromhex(
            TRON_ROUTE_CANARY_FINALITY_BLOCK_HASH
        ),
        route_canary_proof_version=TRON_ROUTE_CANARY_PROOF_VERSION,
        route_canary_proof_source_domain=TRON_ROUTE_CANARY_PROOF_SOURCE_DOMAIN,
        route_canary_used_message_proof=True,
        route_canary_raw_data_owner_matches_transaction=True,
        route_canary_signature_sha256=bytes.fromhex(
            TRON_ROUTE_CANARY_SIGNATURE_SHA256
        ),
        route_canary_signature_recovered_address=bytes.fromhex(
            TRON_ROUTE_CANARY_SIGNATURE_RECOVERED_ADDRESS
        ),
        route_canary_signature_recovers_to_owner=True,
    )


def add_route_canary_transaction_metadata(args):
    args.route_canary_transaction_id = bytes.fromhex(TRON_ROUTE_CANARY_TRANSACTION_ID)
    args.route_canary_transaction_owner_address = bytes.fromhex(
        TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS
    )
    args.route_canary_block_number = TRON_ROUTE_CANARY_BLOCK_NUMBER
    args.route_canary_block_timestamp = TRON_ROUTE_CANARY_BLOCK_TIMESTAMP
    args.route_canary_log_index = 0
    args.route_canary_message_id = bytes.fromhex(TRON_ROUTE_CANARY_MESSAGE_ID)
    args.route_canary_call_data_sha256 = bytes.fromhex(
        TRON_ROUTE_CANARY_CALL_DATA_SHA256
    )
    args.route_canary_payload_hash = bytes.fromhex(TRON_ROUTE_CANARY_PAYLOAD_HASH)
    args.route_canary_target_domain = TRON_ROUTE_CANARY_TARGET_DOMAIN
    args.route_canary_statement_hash = bytes.fromhex(TRON_ROUTE_CANARY_STATEMENT_HASH)
    args.route_canary_commitment_root = bytes.fromhex(
        TRON_ROUTE_CANARY_COMMITMENT_ROOT
    )
    args.route_canary_finality_height = bytes.fromhex(TRON_ROUTE_CANARY_FINALITY_HEIGHT)
    args.route_canary_finality_block_hash = bytes.fromhex(
        TRON_ROUTE_CANARY_FINALITY_BLOCK_HASH
    )
    args.route_canary_proof_version = TRON_ROUTE_CANARY_PROOF_VERSION
    args.route_canary_proof_source_domain = TRON_ROUTE_CANARY_PROOF_SOURCE_DOMAIN
    args.route_canary_used_message_proof = True
    args.route_canary_raw_data_owner_matches_transaction = True
    args.route_canary_signature_sha256 = bytes.fromhex(
        TRON_ROUTE_CANARY_SIGNATURE_SHA256
    )
    args.route_canary_signature_recovered_address = bytes.fromhex(
        TRON_ROUTE_CANARY_SIGNATURE_RECOVERED_ADDRESS
    )
    args.route_canary_signature_recovers_to_owner = True
    return args


def sample_full_toml_cli_args(*, include_route_canary=True):
    args = [
        "--bridge-address",
        "0x1111111111111111111111111111111111111111",
        "--owner-address",
        "0x2222222222222222222222222222222222222222",
        "--network-id",
        "0x" + "33" * 32,
        "--expected-config-hash",
        "0x" + TRON_SOURCE_CONFIG_VECTOR,
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-bridge-emitter-code-hash",
        "0x" + "77" * 32,
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR,
        "--expected-tron-dpos-source-gate-hash",
        "0x" + TRON_DPOS_SOURCE_GATE_HASH_VECTOR,
        "--destination-verifier-address",
        "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        "--destination-verifier-code-hash",
        "0x" + "bb" * 32,
        "--destination-verifier-key-hash",
        "0x" + "cc" * 32,
        "--expected-destination-binding-hash",
        "0x" + TRON_DESTINATION_BINDING_VECTOR,
        "--route-allowlist-hash",
        "0x" + TRON_ROUTE_ALLOWLIST_HASH_VECTOR,
    ]
    if include_route_canary:
        args.extend(
            [
                "--route-canary-evidence-hash",
                "0x" + TRON_ROUTE_CANARY_EVIDENCE_HASH,
                "--route-canary-transaction-id",
                "0x" + TRON_ROUTE_CANARY_TRANSACTION_ID,
                "--route-canary-transaction-owner-address",
                "0x" + TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS,
                "--route-canary-block-number",
                str(TRON_ROUTE_CANARY_BLOCK_NUMBER),
                "--route-canary-block-timestamp",
                str(TRON_ROUTE_CANARY_BLOCK_TIMESTAMP),
                "--route-canary-log-index",
                "0",
                "--route-canary-message-id",
                "0x" + TRON_ROUTE_CANARY_MESSAGE_ID,
                "--route-canary-call-data-sha256",
                "0x" + TRON_ROUTE_CANARY_CALL_DATA_SHA256,
                "--route-canary-payload-hash",
                "0x" + TRON_ROUTE_CANARY_PAYLOAD_HASH,
                "--route-canary-target-domain",
                str(TRON_ROUTE_CANARY_TARGET_DOMAIN),
                "--route-canary-statement-hash",
                "0x" + TRON_ROUTE_CANARY_STATEMENT_HASH,
                "--route-canary-commitment-root",
                "0x" + TRON_ROUTE_CANARY_COMMITMENT_ROOT,
                "--route-canary-finality-height",
                "0x" + TRON_ROUTE_CANARY_FINALITY_HEIGHT,
                "--route-canary-finality-block-hash",
                "0x" + TRON_ROUTE_CANARY_FINALITY_BLOCK_HASH,
                "--route-canary-proof-version",
                str(TRON_ROUTE_CANARY_PROOF_VERSION),
                "--route-canary-proof-source-domain",
                str(TRON_ROUTE_CANARY_PROOF_SOURCE_DOMAIN),
                "--route-canary-used-message-proof",
                "--route-canary-raw-data-owner-matches-transaction",
                "--route-canary-signature-sha256",
                "0x" + TRON_ROUTE_CANARY_SIGNATURE_SHA256,
                "--route-canary-signature-recovered-address",
                "0x" + TRON_ROUTE_CANARY_SIGNATURE_RECOVERED_ADDRESS,
                "--route-canary-signature-recovers-to-owner",
            ]
        )
    return args


def sample_runtime_full_toml_args(module):
    source_runtime_bytecode = bytes.fromhex(TRON_SOURCE_RUNTIME_BYTECODE)
    destination_runtime_bytecode = bytes.fromhex(TRON_DESTINATION_RUNTIME_BYTECODE)
    config_hash = bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR)
    args = sample_full_toml_args()
    args.source_bridge_emitter_code_hash = module.runtime_bytecode_hash(
        source_runtime_bytecode
    )
    args.destination_verifier_code_hash = module.runtime_bytecode_hash(
        destination_runtime_bytecode
    )
    args.expected_destination_binding_hash = module.tron_destination_binding_hash(
        network_id=args.network_id,
        source_domain=args.destination_source_domain,
        target_domain=args.destination_target_domain,
        verifier_address=args.destination_verifier_address,
        verifier_code_hash=args.destination_verifier_code_hash,
        verifier_key_hash=args.destination_verifier_key_hash,
    )
    args.expected_source_verifier_material_hash = (
        module.tron_source_verifier_material_record_hash(args, config_hash)
    )
    args.expected_source_adapter_engine_deployment_hash = (
        module.tron_source_adapter_engine_deployment_record_hash(args, config_hash)
    )
    args.expected_tron_dpos_source_gate_hash = module.tron_dpos_source_gate_hash(
        args,
        config_hash,
    )
    args.route_allowlist_hash = module.tron_route_allowlist_hash(
        source_verifier_material_hash=args.expected_source_verifier_material_hash,
        source_adapter_engine_deployment_hash=(
            args.expected_source_adapter_engine_deployment_hash
        ),
        destination_binding_hash=args.expected_destination_binding_hash,
    )
    args.route_canary_evidence_hash = None
    args.route_canary_evidence_hash = module._route_canary_evidence_hash(
        args,
        route_allowlist_hash=args.route_allowlist_hash,
        destination_binding_hash=args.expected_destination_binding_hash,
        source_verifier_material_hash=args.expected_source_verifier_material_hash,
        source_adapter_engine_deployment_hash=(
            args.expected_source_adapter_engine_deployment_hash
        ),
    )
    args.source_runtime_bytecode = source_runtime_bytecode
    args.destination_runtime_bytecode = destination_runtime_bytecode
    return args


def sample_full_toml_cli_args_with_runtime(module, *, include_route_canary=True):
    values = sample_runtime_full_toml_args(module)
    args = [
        "--bridge-address",
        "0x1111111111111111111111111111111111111111",
        "--owner-address",
        "0x2222222222222222222222222222222222222222",
        "--network-id",
        "0x" + "33" * 32,
        "--expected-config-hash",
        "0x" + TRON_SOURCE_CONFIG_VECTOR,
        "--source-trust-anchor-hash",
        "0x" + "44" * 32,
        "--consensus-verifier-hash",
        "0x" + "55" * 32,
        "--message-inclusion-verifier-hash",
        "0x" + "66" * 32,
        "--source-bridge-runtime-bytecode-hex",
        "0x" + values.source_runtime_bytecode.hex(),
        "--finality-policy-hash",
        "0x" + "88" * 32,
        "--adapter-verifier-vk-hash",
        "0x" + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
        "--deployment-receipt-hash",
        "0x" + "aa" * 32,
        "--expected-source-verifier-material-hash",
        "0x" + values.expected_source_verifier_material_hash.hex(),
        "--expected-source-adapter-engine-deployment-hash",
        "0x" + values.expected_source_adapter_engine_deployment_hash.hex(),
        "--expected-tron-dpos-source-gate-hash",
        "0x" + values.expected_tron_dpos_source_gate_hash.hex(),
        "--destination-verifier-address",
        "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        "--destination-verifier-runtime-bytecode-hex",
        "0x" + values.destination_runtime_bytecode.hex(),
        "--destination-verifier-key-hash",
        "0x" + "cc" * 32,
        "--expected-destination-binding-hash",
        "0x" + values.expected_destination_binding_hash.hex(),
        "--route-allowlist-hash",
        "0x" + values.route_allowlist_hash.hex(),
    ]
    if include_route_canary:
        args.extend(
            [
                "--route-canary-evidence-hash",
                "0x" + values.route_canary_evidence_hash.hex(),
                "--route-canary-transaction-id",
                "0x" + TRON_ROUTE_CANARY_TRANSACTION_ID,
                "--route-canary-transaction-owner-address",
                "0x" + TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS,
                "--route-canary-block-number",
                str(TRON_ROUTE_CANARY_BLOCK_NUMBER),
                "--route-canary-block-timestamp",
                str(TRON_ROUTE_CANARY_BLOCK_TIMESTAMP),
                "--route-canary-log-index",
                "0",
                "--route-canary-message-id",
                "0x" + TRON_ROUTE_CANARY_MESSAGE_ID,
                "--route-canary-call-data-sha256",
                "0x" + TRON_ROUTE_CANARY_CALL_DATA_SHA256,
                "--route-canary-payload-hash",
                "0x" + TRON_ROUTE_CANARY_PAYLOAD_HASH,
                "--route-canary-target-domain",
                str(TRON_ROUTE_CANARY_TARGET_DOMAIN),
                "--route-canary-statement-hash",
                "0x" + TRON_ROUTE_CANARY_STATEMENT_HASH,
                "--route-canary-commitment-root",
                "0x" + TRON_ROUTE_CANARY_COMMITMENT_ROOT,
                "--route-canary-finality-height",
                "0x" + TRON_ROUTE_CANARY_FINALITY_HEIGHT,
                "--route-canary-finality-block-hash",
                "0x" + TRON_ROUTE_CANARY_FINALITY_BLOCK_HASH,
                "--route-canary-proof-version",
                str(TRON_ROUTE_CANARY_PROOF_VERSION),
                "--route-canary-proof-source-domain",
                str(TRON_ROUTE_CANARY_PROOF_SOURCE_DOMAIN),
                "--route-canary-used-message-proof",
                "--route-canary-raw-data-owner-matches-transaction",
                "--route-canary-signature-sha256",
                "0x" + TRON_ROUTE_CANARY_SIGNATURE_SHA256,
                "--route-canary-signature-recovered-address",
                "0x" + TRON_ROUTE_CANARY_SIGNATURE_RECOVERED_ADDRESS,
                "--route-canary-signature-recovers-to-owner",
            ]
        )
    return args, values


def test_tron_source_bridge_config_hash_matches_solidity_abi_vector():
    module = load_evidence_module()

    config_hash = module.tron_source_bridge_config_hash(
        bridge_address=bytes.fromhex("11" * 20),
        network_id=bytes.fromhex("33" * 32),
        source_domain=5,
        target_domain=0,
        owner_address=bytes.fromhex("22" * 20),
    )

    assert config_hash.hex() == TRON_SOURCE_CONFIG_VECTOR


def test_tron_source_message_call_data_matches_tvm_abi_vector():
    module = load_evidence_module()

    call_data = module.tron_source_message_call_data(
        source_domain=5,
        target_domain=0,
        source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
    )

    assert call_data.hex() == TRON_SOURCE_EVENT_CALL_DATA_VECTOR

    try:
        module.tron_source_message_call_data(
            source_domain=5,
            target_domain=1,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
        )
    except ValueError as exc:
        assert "target_domain must be SORA" in str(exc)
    else:
        raise AssertionError("TRON source-call calldata accepted a non-SORA target")


def test_tron_base58check_from_address20_rejects_zero_address():
    module = load_evidence_module()

    address = module.tron_base58check_from_address20(
        bytes.fromhex("11" * 20),
        label="bridge_address",
    )
    assert module.parse_tron_address(address, label="bridge address") == bytes.fromhex(
        "11" * 20
    )

    try:
        module.tron_base58check_from_address20(bytes(20), label="bridge_address")
    except ValueError as exc:
        assert "bridge_address must not be zero" in str(exc)
    else:
        raise AssertionError("zero TRON address was encoded")


def test_tron_source_adapter_verifier_vk_hash_matches_rust_profile_vector():
    module = load_evidence_module()

    vk_hash = module.tron_source_adapter_verifier_vk_hash(
        source_domain=5,
        target_domain=0,
    )

    assert vk_hash.hex() == TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR


def test_tron_source_record_hashes_match_rust_vectors():
    module = load_evidence_module()
    args = sample_full_toml_args()
    config_hash = bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR)

    material_hash = module.tron_source_verifier_material_record_hash(
        args,
        config_hash,
    )
    deployment_hash = module.tron_source_adapter_engine_deployment_record_hash(
        args,
        config_hash,
    )
    gate_hash = module.tron_dpos_source_gate_hash(args, config_hash)

    assert material_hash.hex() == TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    assert deployment_hash.hex() == TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
    assert gate_hash.hex() == TRON_DPOS_SOURCE_GATE_HASH_VECTOR


def test_tron_source_deployment_hash_rejects_noncanonical_adapter_vk_hash():
    module = load_evidence_module()
    args = sample_full_toml_args()
    args.adapter_verifier_vk_hash = bytes.fromhex("99" * 32)

    try:
        module.tron_source_adapter_engine_deployment_record_hash(
            args,
            bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR),
        )
    except ValueError as exc:
        assert "canonical TRON source-adapter verifier profile" in str(exc)
    else:
        raise AssertionError("noncanonical adapter verifier vk hash was accepted")


def test_tron_source_bridge_config_hash_rejects_malformed_direct_material():
    module = load_evidence_module()

    try:
        module.tron_source_bridge_config_hash(
            bridge_address=bytes.fromhex("11" * 19),
            network_id=bytes.fromhex("33" * 32),
            source_domain=5,
            target_domain=0,
            owner_address=bytes.fromhex("22" * 20),
        )
    except ValueError as exc:
        assert "bridge_address must be 20 bytes" in str(exc)
    else:
        raise AssertionError("short direct bridge address was accepted")

    try:
        module.tron_source_bridge_config_hash(
            bridge_address=bytes.fromhex("11" * 20),
            network_id=bytes(32),
            source_domain=5,
            target_domain=0,
            owner_address=bytes.fromhex("22" * 20),
        )
    except ValueError as exc:
        assert "network_id must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct network id was accepted")

    try:
        module.tron_source_bridge_config_hash(
            bridge_address=bytes.fromhex("11" * 20),
            network_id=bytes.fromhex("33" * 32),
            source_domain=5,
            target_domain=1,
            owner_address=bytes.fromhex("22" * 20),
        )
    except ValueError as exc:
        assert "target_domain must be SORA" in str(exc)
    else:
        raise AssertionError("retargeted direct source bridge config was accepted")


def test_tron_destination_binding_hash_matches_rust_solidity_vector():
    module = load_evidence_module()

    binding_hash = module.tron_destination_binding_hash(
        network_id=bytes.fromhex("33" * 32),
        source_domain=0,
        target_domain=5,
        verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        verifier_code_hash=bytes.fromhex("bb" * 32),
        verifier_key_hash=bytes.fromhex("cc" * 32),
    )

    assert binding_hash.hex() == TRON_DESTINATION_BINDING_VECTOR
    assert (
        module.tron_destination_binding_key(
            network_id=bytes.fromhex("33" * 32),
            source_domain=0,
            target_domain=5,
            verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            verifier_code_hash=bytes.fromhex("bb" * 32),
            verifier_key_hash=bytes.fromhex("cc" * 32),
        )
        == TRON_DESTINATION_BINDING_KEY_VECTOR
    )


def test_tron_route_allowlist_hash_matches_rust_profile_vector():
    module = load_evidence_module()

    route_hash = module.tron_route_allowlist_hash(
        source_verifier_material_hash=bytes.fromhex(
            TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        ),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        ),
        destination_binding_hash=bytes.fromhex(TRON_DESTINATION_BINDING_VECTOR),
    )

    assert route_hash.hex() == TRON_ROUTE_ALLOWLIST_HASH_VECTOR


def test_tron_destination_binding_hash_rejects_malformed_direct_material():
    module = load_evidence_module()

    try:
        module.tron_destination_binding_hash(
            network_id=bytes.fromhex("33" * 32),
            source_domain=0,
            target_domain=5,
            verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            verifier_code_hash=bytes(32),
            verifier_key_hash=bytes.fromhex("cc" * 32),
        )
    except ValueError as exc:
        assert "verifier_code_hash must not be zero" in str(exc)
    else:
        raise AssertionError("zero direct verifier code hash was accepted")

    try:
        module.tron_destination_binding_hash(
            network_id=bytes.fromhex("33" * 31),
            source_domain=0,
            target_domain=5,
            verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            verifier_code_hash=bytes.fromhex("bb" * 32),
            verifier_key_hash=bytes.fromhex("cc" * 32),
        )
    except ValueError as exc:
        assert "network_id must be 32 bytes" in str(exc)
    else:
        raise AssertionError("short direct network id was accepted")

    try:
        module.tron_destination_binding_hash(
            network_id=bytes.fromhex("33" * 32),
            source_domain=1,
            target_domain=5,
            verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            verifier_code_hash=bytes.fromhex("bb" * 32),
            verifier_key_hash=bytes.fromhex("cc" * 32),
        )
    except ValueError as exc:
        assert "destination source_domain must be SORA" in str(exc)
    else:
        raise AssertionError("retargeted destination source was accepted")

    try:
        module.tron_destination_binding_hash(
            network_id=bytes.fromhex("33" * 32),
            source_domain=0,
            target_domain=5,
            verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            verifier_code_hash=bytes.fromhex("bb" * 32),
            verifier_key_hash=bytes.fromhex("cc" * 32),
            proof_family="debug-proof-family",
        )
    except ValueError as exc:
        assert "proof_family must be stark-fri-v1" in str(exc)
    else:
        raise AssertionError("non-production destination proof family was accepted")


def test_tron_address_parser_accepts_prefixed_hex_and_rejects_zero():
    module = load_evidence_module()

    assert module.parse_tron_address(
        "0x41" + "11" * 20,
        label="bridge address",
    ) == bytes.fromhex("11" * 20)
    assert module.parse_tron_address(
        "TBXSw8fM4jpQkGc6zZjsVABFpVN7UvXPdV",
        label="bridge address",
    ) == bytes.fromhex("11" * 20)
    assert (
        module.normalize_tron_base58check_address(
            "TBXSw8fM4jpQkGc6zZjsVABFpVN7UvXPdV",
            label="bridge address",
        )
        == "TBXSw8fM4jpQkGc6zZjsVABFpVN7UvXPdV"
    )

    for zero_address in [
        "0x41" + "00" * 20,
        "T9yD14Nj9j7xAB4dbGeiX9h8unkKHxuWwb",
    ]:
        try:
            module.parse_tron_address(zero_address, label="bridge address")
        except module.argparse.ArgumentTypeError as exc:
            assert "must not be zero" in str(exc)
        else:
            raise AssertionError("zero TRON address was accepted")

    for padded_address in [
        " TBXSw8fM4jpQkGc6zZjsVABFpVN7UvXPdV",
        "TBXSw8fM4jpQkGc6zZjsVABFpVN7UvXPdV\n",
    ]:
        try:
            module.parse_tron_address(padded_address, label="bridge address")
        except module.argparse.ArgumentTypeError as exc:
            assert "surrounding whitespace" in str(exc)
        else:
            raise AssertionError("padded TRON address was accepted")

        try:
            module.normalize_tron_base58check_address(
                padded_address,
                label="bridge address",
            )
        except module.argparse.ArgumentTypeError as exc:
            assert "surrounding whitespace" in str(exc)
        else:
            raise AssertionError("padded TRON base58 address was normalized")

    try:
        module.parse_tron_address(
            "0x" + ("11" * 9) + "  " + ("11" * 10),
            label="bridge address",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("internally spaced TRON hex address was accepted")

    for non_canonical_hex_address in [
        "0X41" + "11" * 20,
        "0x41" + "aa" * 19 + "AA",
    ]:
        try:
            module.parse_tron_address(
                non_canonical_hex_address,
                label="bridge address",
            )
        except module.argparse.ArgumentTypeError as exc:
            assert "lowercase" in str(exc)
        else:
            raise AssertionError("non-canonical TRON hex address was accepted")


def test_parse_u32_requires_canonical_decimal_text_and_exact_int():
    module = load_evidence_module()

    assert module.parse_u32("0", label="source domain") == 0
    assert module.parse_u32("5", label="source domain") == 5
    assert module.parse_u32(5, label="source domain") == 5

    for value in ["05", "0x5", "+5", " 5", "\uff15", False, True]:
        try:
            module.parse_u32(value, label="source domain")
        except module.argparse.ArgumentTypeError as exc:
            assert "source domain must be a u32" in str(exc)
        else:
            raise AssertionError(f"non-canonical u32 value {value!r} was accepted")


def test_parse_hex_bytes_rejects_padded_tron_source_material():
    module = load_evidence_module()

    assert module.parse_hex_bytes(
        "0x" + "33" * 32,
        label="network id",
        byte_length=32,
    ) == bytes.fromhex("33" * 32)

    try:
        module.parse_hex_bytes(
            " 0x" + "33" * 32,
            label="network id",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("padded TRON source network id was accepted")

    try:
        module.parse_hex_bytes(
            "0x" + ("33" * 15) + "  " + ("33" * 16),
            label="network id",
            byte_length=32,
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "must not contain whitespace" in str(exc)
    else:
        raise AssertionError("internally spaced TRON source network id was accepted")

    for non_canonical_hex in [
        "0X" + "33" * 32,
        "0x" + ("33" * 31) + "AA",
    ]:
        try:
            module.parse_hex_bytes(
                non_canonical_hex,
                label="network id",
                byte_length=32,
            )
        except module.argparse.ArgumentTypeError as exc:
            assert "lowercase" in str(exc)
        else:
            raise AssertionError("non-canonical TRON source network id was accepted")


def test_parse_runtime_bytecode_hex_rejects_padded_inline_text(tmp_path):
    module = load_evidence_module()
    runtime_bytecode = bytes.fromhex("6001600055")

    assert module.parse_runtime_bytecode_hex(
        "0x" + runtime_bytecode.hex(),
        label="source bridge runtime bytecode",
    ) == runtime_bytecode

    for padded_runtime in [
        " 0x" + runtime_bytecode.hex(),
        "0x" + runtime_bytecode.hex() + "\n",
        "0x6001 600055",
    ]:
        try:
            module.parse_runtime_bytecode_hex(
                padded_runtime,
                label="source bridge runtime bytecode",
            )
        except module.argparse.ArgumentTypeError as exc:
            assert "whitespace" in str(exc)
        else:
            raise AssertionError("padded inline runtime bytecode was accepted")

    for non_canonical_runtime in [
        "0X6001600055",
        "0x600Aef",
    ]:
        try:
            module.parse_runtime_bytecode_hex(
                non_canonical_runtime,
                label="source bridge runtime bytecode",
            )
        except module.argparse.ArgumentTypeError as exc:
            assert "lowercase" in str(exc)
        else:
            raise AssertionError("non-canonical inline runtime bytecode was accepted")

    bytecode_file = tmp_path / "runtime.hex"
    bytecode_file.write_text("0x6001\n600055\n", encoding="utf-8")
    assert module.parse_runtime_bytecode_file(
        str(bytecode_file),
        label="source bridge runtime bytecode",
    ) == runtime_bytecode

    uppercase_file = tmp_path / "uppercase-runtime.hex"
    uppercase_file.write_text("0X6001600055\n", encoding="utf-8")
    try:
        module.parse_runtime_bytecode_file(
            str(uppercase_file),
            label="source bridge runtime bytecode",
        )
    except module.argparse.ArgumentTypeError as exc:
        assert "lowercase" in str(exc)
    else:
        raise AssertionError("non-canonical runtime bytecode file was accepted")


def test_tron_source_bridge_hashes_reject_boolean_domain_values():
    module = load_evidence_module()
    config_hash = bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR)

    try:
        module.tron_source_bridge_config_hash(
            bridge_address=bytes.fromhex("11" * 20),
            network_id=bytes.fromhex("33" * 32),
            source_domain=5,
            target_domain=False,
            owner_address=bytes.fromhex("22" * 20),
        )
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean TRON target domain reached config hashing")

    try:
        module.tron_source_message_call_data(
            source_domain=5,
            target_domain=False,
            source_event_digest=bytes.fromhex(TRON_SOURCE_EVENT_DIGEST_VECTOR),
        )
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean TRON target domain reached calldata hashing")

    try:
        module.tron_source_adapter_verifier_vk_hash(
            source_domain=5,
            target_domain=False,
        )
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean TRON target domain reached vk hashing")

    args = sample_full_toml_args()
    args.target_domain = False
    try:
        module.tron_source_adapter_engine_deployment_record_hash(args, config_hash)
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean TRON target domain reached deployment hashing")

    try:
        module.tron_destination_binding_hash(
            network_id=bytes.fromhex("33" * 32),
            source_domain=False,
            target_domain=5,
            verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            verifier_code_hash=bytes.fromhex("bb" * 32),
            verifier_key_hash=bytes.fromhex("cc" * 32),
        )
    except ValueError as exc:
        assert "source_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("boolean TRON destination source domain reached hashing")


def test_toml_rendering_carries_mainnet_profile_ids_and_config_hash():
    module = load_evidence_module()
    config_hash = bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR)
    args = SimpleNamespace(
        source_domain=5,
        target_domain=0,
        bridge_address=bytes.fromhex("11" * 20),
        owner_address=bytes.fromhex("22" * 20),
        network_id=bytes.fromhex("33" * 32),
        expected_config_hash=config_hash,
        source_trust_anchor_hash=bytes.fromhex("44" * 32),
        consensus_verifier_hash=bytes.fromhex("55" * 32),
        message_inclusion_verifier_hash=bytes.fromhex("66" * 32),
        source_bridge_emitter_code_hash=bytes.fromhex("77" * 32),
        finality_policy_hash=bytes.fromhex("88" * 32),
        adapter_verifier_vk_hash=bytes.fromhex(
            TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        ),
        deployment_receipt_hash=bytes.fromhex("aa" * 32),
        expected_source_verifier_material_hash=bytes.fromhex(
            TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        ),
        expected_source_adapter_engine_deployment_hash=bytes.fromhex(
            TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        ),
        expected_tron_dpos_source_gate_hash=bytes.fromhex(
            TRON_DPOS_SOURCE_GATE_HASH_VECTOR
        ),
        destination_verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        destination_verifier_code_hash=bytes.fromhex("bb" * 32),
        destination_verifier_key_hash=bytes.fromhex("cc" * 32),
        destination_source_domain=0,
        destination_target_domain=5,
        destination_proof_family="stark-fri-v1",
        expected_destination_binding_hash=bytes.fromhex(
            TRON_DESTINATION_BINDING_VECTOR
        ),
        route_allowlist_hash=bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR),
        route_canary_evidence_hash=bytes.fromhex(TRON_ROUTE_CANARY_EVIDENCE_HASH),
        route_canary_transaction_id=bytes.fromhex(TRON_ROUTE_CANARY_TRANSACTION_ID),
        route_canary_transaction_owner_address=bytes.fromhex(
            TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS
        ),
        route_canary_block_number=TRON_ROUTE_CANARY_BLOCK_NUMBER,
        route_canary_block_timestamp=TRON_ROUTE_CANARY_BLOCK_TIMESTAMP,
        route_canary_log_index=0,
        route_canary_message_id=bytes.fromhex(TRON_ROUTE_CANARY_MESSAGE_ID),
        route_canary_call_data_sha256=bytes.fromhex(
            TRON_ROUTE_CANARY_CALL_DATA_SHA256
        ),
        route_canary_payload_hash=bytes.fromhex(TRON_ROUTE_CANARY_PAYLOAD_HASH),
        route_canary_target_domain=TRON_ROUTE_CANARY_TARGET_DOMAIN,
        route_canary_statement_hash=bytes.fromhex(TRON_ROUTE_CANARY_STATEMENT_HASH),
        route_canary_commitment_root=bytes.fromhex(
            TRON_ROUTE_CANARY_COMMITMENT_ROOT
        ),
        route_canary_finality_height=bytes.fromhex(TRON_ROUTE_CANARY_FINALITY_HEIGHT),
        route_canary_finality_block_hash=bytes.fromhex(
            TRON_ROUTE_CANARY_FINALITY_BLOCK_HASH
        ),
        route_canary_proof_version=TRON_ROUTE_CANARY_PROOF_VERSION,
        route_canary_proof_source_domain=TRON_ROUTE_CANARY_PROOF_SOURCE_DOMAIN,
        route_canary_used_message_proof=True,
        route_canary_raw_data_owner_matches_transaction=True,
        route_canary_signature_sha256=bytes.fromhex(
            TRON_ROUTE_CANARY_SIGNATURE_SHA256
        ),
        route_canary_signature_recovered_address=bytes.fromhex(
            TRON_ROUTE_CANARY_SIGNATURE_RECOVERED_ADDRESS
        ),
        route_canary_signature_recovers_to_owner=True,
    )

    rendered = module.render_toml(args, config_hash)

    assert (
        '# sccp_tron_source_verifier_material_hash = "0x'
        + TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        + '"'
        in rendered
    )
    assert '# sccp_tron_source_bridge_address = "0x' + "11" * 20 + '"' in rendered
    assert (
        '# sccp_tron_source_bridge_runtime_code_hash = "0x'
        + "77" * 32
        + '"'
        in rendered
    )
    assert (
        '# sccp_tron_source_bridge_config_hash = "0x'
        + TRON_SOURCE_CONFIG_VECTOR
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_verifier_materials]]" in rendered
    assert (
        '# sccp_tron_source_adapter_engine_deployment_hash = "0x'
        + TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        + '"'
        in rendered
    )
    assert (
        '# sccp_tron_dpos_source_gate_hash = "0x'
        + TRON_DPOS_SOURCE_GATE_HASH_VECTOR
        + '"'
        in rendered
    )
    assert "[[zk.sccp_source_adapter_engine_deployments]]" in rendered
    assert (
        'tron_dpos_source_gate_hash = "0x'
        + TRON_DPOS_SOURCE_GATE_HASH_VECTOR
        + '"'
        in rendered
    )
    assert (
        'source_trust_anchor_id = "sccp:tron:source-trust-anchor:mainnet-witness-schedule:v1"'
        in rendered
    )
    assert (
        'source_bridge_config_hash = "0xe986dd67bfa2307b4e00cf46bde41a88003a55c5b7fea311fa106614b2252f9d"'
        in rendered
    )
    assert 'source_bridge_network_id = "0x' + "33" * 32 + '"' in rendered
    assert 'source_bridge_owner_address = "0x' + "22" * 20 + '"' in rendered
    assert 'source_bridge_emitter_code_hash = "0x' + "77" * 32 + '"' in rendered
    assert (
        'adapter_verifier_vk_hash = "0x'
        + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        + '"'
        in rendered
    )
    assert 'deployment_receipt_hash = "0x' + "aa" * 32 + '"' in rendered

    full_rendered = module.render_full_toml(args, config_hash)
    assert "[[zk.sccp_destination_rollouts]]" in full_rendered
    assert "[[zk.sccp_route_allowlists]]" in full_rendered
    assert '# sccp_tron_source_bridge_address = "0x' + "11" * 20 + '"' in full_rendered
    assert (
        '# sccp_tron_source_bridge_runtime_code_hash = "0x'
        + "77" * 32
        + '"'
        in full_rendered
    )
    assert (
        '# sccp_tron_source_bridge_config_hash = "0x'
        + TRON_SOURCE_CONFIG_VECTOR
        + '"'
        in full_rendered
    )
    assert full_rendered.count("# sccp_tron_destination_binding_hash = ") == 1
    assert (
        '# sccp_tron_destination_binding_hash = "0x'
        + TRON_DESTINATION_BINDING_VECTOR
        + '"'
        in full_rendered
    )
    assert (
        '# sccp_tron_destination_binding_key = "'
        + TRON_DESTINATION_BINDING_KEY_VECTOR
        + '"'
        in full_rendered
    )
    assert (
        '# sccp_tron_destination_verifier_address = "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"'
        in full_rendered
    )
    assert (
        '# sccp_tron_destination_verifier_runtime_code_hash = "0x'
        + "bb" * 32
        + '"'
        in full_rendered
    )
    assert (
        '# sccp_tron_destination_verifier_key_hash = "0x'
        + "cc" * 32
        + '"'
        in full_rendered
    )
    assert "# sccp_tron_destination_verifier_backend_hash" in full_rendered
    assert "# sccp_tron_destination_proof_family_hash" in full_rendered
    assert 'destination_network_id = "0x' + "33" * 32 + '"' in full_rendered
    assert full_rendered.count('verifier_code_hash = "0x' + "bb" * 32 + '"') == 1
    assert (
        'destination_binding_key = "'
        + TRON_DESTINATION_BINDING_KEY_VECTOR
        + '"'
        in full_rendered
    )
    assert (
        'destination_binding_hash = "0x'
        + TRON_DESTINATION_BINDING_VECTOR
        + '"'
        in full_rendered
    )
    assert 'verifier_plan = "TronContractGroth16Bn254"' in full_rendered
    assert (
        'anchor_id = "sccp:tron:destination-anchor:tron-mainnet:v1"'
        in full_rendered
    )
    assert (
        'route_allowlist_id = "sccp:tron:route-allowlist:tron-mainnet:v1"'
        in full_rendered
    )
    assert 'verifier_key_hash = "0x' + "cc" * 32 + '"' in full_rendered
    assert (
        'route_allowlist_hash = "0x'
        + TRON_ROUTE_ALLOWLIST_HASH_VECTOR
        + '"'
        in full_rendered
    )
    assert '# sccp_route_canary_status = "passed"' in full_rendered
    assert 'route_canary_status = "passed"' in full_rendered
    assert (
        '# sccp_route_canary_evidence_hash = "0x'
        + TRON_ROUTE_CANARY_EVIDENCE_HASH
        + '"'
        in full_rendered
    )
    assert (
        'route_canary_evidence_hash = "0x'
        + TRON_ROUTE_CANARY_EVIDENCE_HASH
        + '"'
        in full_rendered
    )
    assert (
        'tron_route_canary_transaction_id = "0x'
        + TRON_ROUTE_CANARY_TRANSACTION_ID
        + '"'
        in full_rendered
    )
    assert (
        'tron_route_canary_transaction_owner_address = "0x'
        + TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS
        + '"'
        in full_rendered
    )
    assert (
        f'tron_route_canary_block_number = {TRON_ROUTE_CANARY_BLOCK_NUMBER}'
        in full_rendered
    )
    assert (
        f'tron_route_canary_block_timestamp = {TRON_ROUTE_CANARY_BLOCK_TIMESTAMP}'
        in full_rendered
    )
    assert "tron_route_canary_log_index = 0" in full_rendered
    assert (
        'tron_route_canary_message_id = "0x'
        + TRON_ROUTE_CANARY_MESSAGE_ID
        + '"'
        in full_rendered
    )
    assert (
        'tron_route_canary_statement_hash = "0x'
        + TRON_ROUTE_CANARY_STATEMENT_HASH
        + '"'
        in full_rendered
    )
    assert (
        'tron_route_canary_commitment_root = "0x'
        + TRON_ROUTE_CANARY_COMMITMENT_ROOT
        + '"'
        in full_rendered
    )
    assert "tron_route_canary_used_message_proof = true" in full_rendered
    assert (
        "tron_route_canary_raw_data_owner_matches_transaction = true"
        in full_rendered
    )
    assert (
        '# sccp_route_canary_route_allowlist_hash = "0x'
        + TRON_ROUTE_ALLOWLIST_HASH_VECTOR
        + '"'
        in full_rendered
    )
    assert (
        '# sccp_route_canary_route_allowlist_hash = #'
        not in full_rendered
    )
    assert (
        '# sccp_route_canary_destination_binding_hash = "0x'
        + TRON_DESTINATION_BINDING_VECTOR
        + '"'
        in full_rendered
    )


def test_toml_rendering_rejects_reused_source_role_hashes():
    module = load_evidence_module()
    args = sample_full_toml_args()
    args.source_bridge_emitter_code_hash = args.finality_policy_hash

    try:
        module.render_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "source_bridge_emitter_code_hash matches finality_policy_hash" in str(exc)
    else:
        raise AssertionError("TRON TOML accepted reused source-adapter role hashes")


def test_direct_record_hashes_reject_reused_source_role_hashes():
    module = load_evidence_module()
    config_hash = bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR)

    material_args = sample_full_toml_args()
    material_args.source_bridge_emitter_code_hash = material_args.finality_policy_hash
    try:
        module.tron_source_verifier_material_record_hash(material_args, config_hash)
    except ValueError as exc:
        assert "source_bridge_emitter_code_hash matches finality_policy_hash" in str(
            exc
        )
    else:
        raise AssertionError("TRON material hash accepted reused role hashes")

    deployment_args = sample_full_toml_args()
    deployment_args.deployment_receipt_hash = deployment_args.adapter_verifier_vk_hash
    try:
        module.tron_source_adapter_engine_deployment_record_hash(
            deployment_args,
            config_hash,
        )
    except ValueError as exc:
        assert "deployment_receipt_hash matches adapter_verifier_vk_hash" in str(exc)
    else:
        raise AssertionError("TRON deployment hash accepted reused role hashes")


def test_direct_toml_renderers_reject_runtime_bytecode_hash_mismatch():
    module = load_evidence_module()
    config_hash = bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR)

    for renderer in (module.render_toml, module.render_full_toml):
        args = sample_full_toml_args()
        args.source_bridge_runtime_bytecode_hex = bytes.fromhex("6001600055")
        args.source_bridge_runtime_bytecode_file = None
        args.destination_verifier_runtime_bytecode_hex = None
        args.destination_verifier_runtime_bytecode_file = None

        try:
            renderer(args, config_hash)
        except ValueError as exc:
            assert "source-bridge-emitter-code-hash does not match" in str(exc)
        else:
            raise AssertionError(
                f"{renderer.__name__} accepted a mismatched source bridge bytecode hash"
            )


def test_toml_rendering_rejects_template_source_component_hashes():
    module = load_evidence_module()
    for field, (component_id, component_kind) in module.TRON_TEMPLATE_COMPONENTS.items():
        args = sample_full_toml_args()
        setattr(
            args,
            field,
            module._tron_template_component_hash(component_id, component_kind),
        )
        label = field.replace("_", " ")

        try:
            module.render_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
        except ValueError as exc:
            assert f"template-derived {label}" in str(exc)
        else:
            raise AssertionError(f"template-derived TRON {label} was accepted")


def test_direct_record_hashes_reject_template_source_component_hashes():
    module = load_evidence_module()
    config_hash = bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR)

    for field, (component_id, component_kind) in module.TRON_TEMPLATE_COMPONENTS.items():
        template_hash = module._tron_template_component_hash(
            component_id,
            component_kind,
        )
        label = field.replace("_", " ")

        material_args = sample_full_toml_args()
        setattr(material_args, field, template_hash)
        try:
            module.tron_source_verifier_material_record_hash(material_args, config_hash)
        except ValueError as exc:
            assert f"template-derived {label}" in str(exc)
        else:
            raise AssertionError(
                f"direct material hash accepted template {label}"
            )

        deployment_args = sample_full_toml_args()
        setattr(deployment_args, field, template_hash)
        try:
            module.tron_source_adapter_engine_deployment_record_hash(
                deployment_args,
                config_hash,
            )
        except ValueError as exc:
            assert f"template-derived {label}" in str(exc)
        else:
            raise AssertionError(
                f"direct deployment hash accepted template {label}"
            )


def test_full_toml_rendering_rejects_retargeted_destination_lane():
    module = load_evidence_module()
    args = sample_full_toml_args()
    args.destination_source_domain = 1

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "SORA -> TRON" in str(exc)
    else:
        raise AssertionError("full TOML accepted a non-SORA destination source")


def test_full_toml_rendering_rejects_non_production_destination_proof_family():
    module = load_evidence_module()
    args = sample_full_toml_args()
    args.destination_proof_family = "debug-proof-family"

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "destination-proof-family stark-fri-v1" in str(exc)
    else:
        raise AssertionError("full TOML accepted a non-production proof family")


def test_full_toml_rendering_rejects_zero_route_allowlist_hash():
    module = load_evidence_module()
    args = sample_full_toml_args()
    args.route_allowlist_hash = bytes(32)

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "route_allowlist_hash must not be zero" in str(exc)
    else:
        raise AssertionError("full TOML accepted a zero route allowlist hash")


def test_full_toml_rendering_rejects_route_allowlist_hash_drift():
    module = load_evidence_module()
    args = sample_full_toml_args()
    args.route_allowlist_hash = bytes.fromhex("dd" * 32)

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "canonical source, deployment, and destination evidence" in str(exc)
    else:
        raise AssertionError("full TOML accepted a drifted route allowlist hash")


def test_full_toml_rendering_requires_route_canary_evidence():
    module = load_evidence_module()
    args = sample_full_toml_args()
    args.route_canary_evidence_hash = None
    args.route_canary_transaction_id = None
    args.route_canary_transaction_owner_address = None
    args.route_canary_block_number = None
    args.route_canary_block_timestamp = None
    args.route_canary_log_index = None
    args.route_canary_message_id = None
    args.route_canary_call_data_sha256 = None
    args.route_canary_payload_hash = None
    args.route_canary_target_domain = None
    args.route_canary_statement_hash = None
    args.route_canary_commitment_root = None
    args.route_canary_finality_height = None
    args.route_canary_finality_block_hash = None
    args.route_canary_proof_version = None
    args.route_canary_proof_source_domain = None
    args.route_canary_used_message_proof = None
    args.route_canary_raw_data_owner_matches_transaction = None
    args.route_canary_signature_sha256 = None
    args.route_canary_signature_recovered_address = None
    args.route_canary_signature_recovers_to_owner = None

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "--route-canary-transaction-id" in str(exc)
    else:
        raise AssertionError("full TOML accepted missing route canary evidence")


def test_full_toml_rendering_rejects_hash_only_route_canary_evidence():
    module = load_evidence_module()
    args = sample_full_toml_args()
    args.route_canary_transaction_id = None
    args.route_canary_transaction_owner_address = None
    args.route_canary_block_number = None
    args.route_canary_block_timestamp = None
    args.route_canary_log_index = None
    args.route_canary_message_id = None
    args.route_canary_call_data_sha256 = None
    args.route_canary_payload_hash = None
    args.route_canary_target_domain = None
    args.route_canary_statement_hash = None
    args.route_canary_commitment_root = None
    args.route_canary_finality_height = None
    args.route_canary_finality_block_hash = None
    args.route_canary_proof_version = None
    args.route_canary_proof_source_domain = None
    args.route_canary_used_message_proof = None
    args.route_canary_raw_data_owner_matches_transaction = None
    args.route_canary_signature_sha256 = None
    args.route_canary_signature_recovered_address = None
    args.route_canary_signature_recovers_to_owner = None

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "--route-canary-transaction-id" in str(exc)
    else:
        raise AssertionError("full TOML accepted hash-only route canary evidence")


def test_full_toml_rendering_binds_route_canary_call_transcript_metadata():
    module = load_evidence_module()

    for attr_name, value, expected in (
        (
            "route_canary_call_data_sha256",
            bytes.fromhex("01" * 32),
            "route_canary_evidence_hash does not match",
        ),
        (
            "route_canary_payload_hash",
            bytes.fromhex("02" * 32),
            "route_canary_evidence_hash does not match",
        ),
        (
            "route_canary_block_number",
            TRON_ROUTE_CANARY_BLOCK_NUMBER + 1,
            "route_canary_evidence_hash does not match",
        ),
        (
            "route_canary_block_timestamp",
            TRON_ROUTE_CANARY_BLOCK_TIMESTAMP + 1,
            "route_canary_evidence_hash does not match",
        ),
        (
            "route_canary_finality_height",
            bytes.fromhex("03" * 32),
            "route_canary_evidence_hash does not match",
        ),
        (
            "route_canary_finality_block_hash",
            bytes.fromhex("04" * 32),
            "route_canary_evidence_hash does not match",
        ),
        (
            "route_canary_message_id",
            bytes.fromhex("05" * 32),
            "route_canary_evidence_hash does not match",
        ),
        (
            "route_canary_statement_hash",
            bytes.fromhex("06" * 32),
            "route_canary_evidence_hash does not match",
        ),
        (
            "route_canary_commitment_root",
            bytes.fromhex("07" * 32),
            "route_canary_evidence_hash does not match",
        ),
        (
            "route_canary_target_domain",
            6,
            "route canary target domain must match destination_target_domain",
        ),
        (
            "route_canary_proof_version",
            2,
            "route canary proof version must be 1",
        ),
        (
            "route_canary_proof_source_domain",
            1,
            "route canary proof source domain must match destination_source_domain",
        ),
    ):
        args = sample_full_toml_args()
        setattr(args, attr_name, value)
        try:
            module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
        except ValueError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(
                f"full TOML accepted drifted {attr_name} in route canary evidence"
            )


def test_full_toml_rendering_binds_route_canary_signature_metadata():
    module = load_evidence_module()

    signature_hash_args = sample_full_toml_args()
    signature_hash_args.route_canary_signature_sha256 = bytes.fromhex("6b" * 32)
    try:
        module.render_full_toml(
            signature_hash_args,
            bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR),
        )
    except ValueError as exc:
        assert "route_canary_evidence_hash does not match" in str(exc)
    else:
        raise AssertionError("full TOML accepted drifted route canary signature hash")

    recovered_address_args = add_route_canary_transaction_metadata(
        sample_full_toml_args()
    )
    recovered_address_args.route_canary_signature_recovered_address = bytes.fromhex(
        "41" + "97" * 20
    )
    try:
        module.render_full_toml(
            recovered_address_args,
            bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR),
        )
    except ValueError as exc:
        assert "signature recovered address must match" in str(exc)
    else:
        raise AssertionError(
            "full TOML accepted drifted route canary recovered signer"
        )


def test_full_toml_rendering_rejects_route_canary_transcript_hash_reuse():
    module = load_evidence_module()

    for attr_name, source_attr_name in (
        ("route_canary_message_id", "route_canary_transaction_id"),
        ("route_canary_payload_hash", "route_canary_call_data_sha256"),
        ("route_canary_commitment_root", "route_canary_statement_hash"),
        ("route_canary_finality_height", "route_canary_transaction_id"),
        ("route_canary_signature_sha256", "route_canary_finality_block_hash"),
    ):
        args = sample_full_toml_args()
        setattr(args, attr_name, getattr(args, source_attr_name))

        try:
            module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
        except ValueError as exc:
            assert "TRON route canary transcript hashes must be distinct" in str(exc)
        else:
            raise AssertionError(
                "full TOML accepted reused route canary transcript hash "
                f"{attr_name}"
            )


def test_route_canary_transaction_hash_requires_production_destination_lane():
    module = load_evidence_module()

    for source_domain, target_domain, proof_source_domain, canary_target_domain in (
        (1, module.SCCP_DOMAIN_TRON, 1, module.SCCP_DOMAIN_TRON),
        (
            module.SCCP_DOMAIN_SORA,
            module.SCCP_DOMAIN_SORA,
            module.SCCP_DOMAIN_SORA,
            module.SCCP_DOMAIN_SORA,
        ),
    ):
        args = add_route_canary_transaction_metadata(sample_full_toml_args())
        args.destination_source_domain = source_domain
        args.destination_target_domain = target_domain
        args.route_canary_proof_source_domain = proof_source_domain
        args.route_canary_target_domain = canary_target_domain

        try:
            module._route_canary_evidence_hash(
                args,
                route_allowlist_hash=bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR),
                destination_binding_hash=bytes.fromhex(TRON_DESTINATION_BINDING_VECTOR),
                source_verifier_material_hash=bytes.fromhex(
                    TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
                ),
                source_adapter_engine_deployment_hash=bytes.fromhex(
                    TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
                ),
            )
        except ValueError as exc:
            assert "production SORA -> TRON destination lane" in str(exc)
        else:
            raise AssertionError(
                "route canary transaction hash accepted a non-production "
                "destination lane"
            )


def test_route_canary_transaction_hash_requires_canonical_binding_material():
    module = load_evidence_module()

    cases = (
        ("route_allowlist_hash", bytes(32), "route_allowlist_hash must not be zero"),
        (
            "route_allowlist_hash",
            bytes.fromhex("dd" * 32),
            "route_allowlist_hash does not match canonical",
        ),
        (
            "destination_binding_hash",
            bytes(32),
            "destination_binding_hash must not be zero",
        ),
        (
            "destination_binding_hash",
            bytes.fromhex("ee" * 32),
            "destination_binding_hash does not match canonical destination binding",
        ),
        ("network_id", bytes(32), "network_id must not be zero"),
        (
            "destination_proof_family",
            "not-stark",
            "proof_family must be stark-fri-v1",
        ),
    )
    for field, value, expected in cases:
        args = add_route_canary_transaction_metadata(sample_full_toml_args())
        route_allowlist_hash = bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR)
        destination_binding_hash = bytes.fromhex(TRON_DESTINATION_BINDING_VECTOR)
        if field == "route_allowlist_hash":
            route_allowlist_hash = value
        elif field == "destination_binding_hash":
            destination_binding_hash = value
            if any(destination_binding_hash):
                route_allowlist_hash = module.tron_route_allowlist_hash(
                    source_verifier_material_hash=bytes.fromhex(
                        TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
                    ),
                    source_adapter_engine_deployment_hash=bytes.fromhex(
                        TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
                    ),
                    destination_binding_hash=destination_binding_hash,
                )
        else:
            setattr(args, field, value)

        try:
            module._route_canary_evidence_hash(
                args,
                route_allowlist_hash=route_allowlist_hash,
                destination_binding_hash=destination_binding_hash,
                source_verifier_material_hash=bytes.fromhex(
                    TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
                ),
                source_adapter_engine_deployment_hash=bytes.fromhex(
                    TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
                ),
            )
        except ValueError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(
                f"route canary transaction hash accepted invalid {field}"
            )


def test_full_toml_rendering_derives_route_canary_from_transaction_metadata():
    module = load_evidence_module()
    args = add_route_canary_transaction_metadata(sample_full_toml_args())
    args.route_canary_evidence_hash = None

    full_rendered = module.render_full_toml(
        args,
        bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR),
    )

    expected_hash = module._route_canary_evidence_hash(
        args,
        route_allowlist_hash=bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR),
        destination_binding_hash=bytes.fromhex(TRON_DESTINATION_BINDING_VECTOR),
        source_verifier_material_hash=bytes.fromhex(
            TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
        ),
        source_adapter_engine_deployment_hash=bytes.fromhex(
            TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
        ),
    )
    assert expected_hash is not None
    assert (
        '# sccp_route_canary_evidence_hash = "0x'
        + expected_hash.hex()
        + '"'
        in full_rendered
    )
    assert (
        '# sccp_tron_route_canary_transaction_id = "0x'
        + TRON_ROUTE_CANARY_TRANSACTION_ID
        + '"'
        in full_rendered
    )
    assert (
        'tron_route_canary_transaction_id = "0x'
        + TRON_ROUTE_CANARY_TRANSACTION_ID
        + '"'
        in full_rendered
    )
    assert (
        "# sccp_tron_route_canary_transaction_owner_address = "
        f'"0x{TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS}"'
        in full_rendered
    )
    assert (
        "tron_route_canary_transaction_owner_address = "
        f'"0x{TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS}"'
        in full_rendered
    )
    assert (
        "# sccp_tron_route_canary_block_number = "
        f'"{TRON_ROUTE_CANARY_BLOCK_NUMBER}"'
        in full_rendered
    )
    assert (
        "tron_route_canary_block_number = "
        f"{TRON_ROUTE_CANARY_BLOCK_NUMBER}"
        in full_rendered
    )
    assert (
        "# sccp_tron_route_canary_block_timestamp = "
        f'"{TRON_ROUTE_CANARY_BLOCK_TIMESTAMP}"'
        in full_rendered
    )
    assert (
        "tron_route_canary_block_timestamp = "
        f"{TRON_ROUTE_CANARY_BLOCK_TIMESTAMP}"
        in full_rendered
    )
    assert '# sccp_tron_route_canary_log_index = "0"' in full_rendered
    assert "tron_route_canary_log_index = 0" in full_rendered
    assert (
        '# sccp_tron_route_canary_message_id = "0x'
        + TRON_ROUTE_CANARY_MESSAGE_ID
        + '"'
        in full_rendered
    )
    assert (
        'tron_route_canary_message_id = "0x'
        + TRON_ROUTE_CANARY_MESSAGE_ID
        + '"'
        in full_rendered
    )
    assert (
        'tron_route_canary_statement_hash = "0x'
        + TRON_ROUTE_CANARY_STATEMENT_HASH
        + '"'
        in full_rendered
    )
    assert (
        'tron_route_canary_commitment_root = "0x'
        + TRON_ROUTE_CANARY_COMMITMENT_ROOT
        + '"'
        in full_rendered
    )
    assert '# sccp_tron_route_canary_used_message_proof = "true"' in full_rendered
    assert "tron_route_canary_used_message_proof = true" in full_rendered
    assert (
        '# sccp_tron_route_canary_raw_data_owner_matches_transaction = "true"'
        in full_rendered
    )
    assert (
        "tron_route_canary_raw_data_owner_matches_transaction = true"
        in full_rendered
    )
    assert '# sccp_tron_route_canary_signature_sha256 = "0x' in full_rendered
    assert (
        "tron_route_canary_signature_sha256 = "
        f'"0x{TRON_ROUTE_CANARY_SIGNATURE_SHA256}"'
        in full_rendered
    )
    assert (
        "# sccp_tron_route_canary_signature_recovered_address = "
        f'"0x{TRON_ROUTE_CANARY_SIGNATURE_RECOVERED_ADDRESS}"'
        in full_rendered
    )
    assert (
        "tron_route_canary_signature_recovered_address = "
        f'"0x{TRON_ROUTE_CANARY_SIGNATURE_RECOVERED_ADDRESS}"'
        in full_rendered
    )
    assert (
        '# sccp_tron_route_canary_signature_recovers_to_owner = "true"'
        in full_rendered
    )
    assert "tron_route_canary_signature_recovers_to_owner = true" in full_rendered


def test_full_toml_rendering_requires_route_canary_used_message_state():
    module = load_evidence_module()
    args = add_route_canary_transaction_metadata(sample_full_toml_args())
    args.route_canary_used_message_proof = None

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "--route-canary-used-message-proof" in str(exc)
    else:
        raise AssertionError("full TOML accepted route canary without used state")


def test_full_toml_rendering_requires_route_canary_block_metadata():
    module = load_evidence_module()

    for field, expected in (
        ("route_canary_block_number", "--route-canary-block-number"),
        ("route_canary_block_timestamp", "--route-canary-block-timestamp"),
    ):
        args = add_route_canary_transaction_metadata(sample_full_toml_args())
        setattr(args, field, None)

        try:
            module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
        except ValueError as exc:
            assert expected in str(exc)
        else:
            raise AssertionError(
                f"full TOML accepted route canary without {field}"
            )


def test_full_toml_rendering_rejects_zero_route_canary_block_number():
    module = load_evidence_module()
    args = add_route_canary_transaction_metadata(sample_full_toml_args())
    args.route_canary_block_number = 0

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "route_canary_block_number must be a positive u64" in str(exc)
    else:
        raise AssertionError("full TOML accepted a zero route canary block number")


def test_full_toml_rendering_requires_route_canary_raw_owner_binding():
    module = load_evidence_module()
    args = add_route_canary_transaction_metadata(sample_full_toml_args())
    args.route_canary_raw_data_owner_matches_transaction = None

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "--route-canary-raw-data-owner-matches-transaction" in str(exc)
    else:
        raise AssertionError(
            "full TOML accepted route canary without raw_data owner binding"
        )


def test_full_toml_rendering_requires_route_canary_signature_recovery():
    module = load_evidence_module()
    args = add_route_canary_transaction_metadata(sample_full_toml_args())
    args.route_canary_signature_recovers_to_owner = None

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "--route-canary-signature-recovers-to-owner" in str(exc)
    else:
        raise AssertionError(
            "full TOML accepted route canary without signature recovery"
        )


def test_full_toml_rendering_rejects_route_canary_signature_owner_mismatch():
    module = load_evidence_module()
    args = add_route_canary_transaction_metadata(sample_full_toml_args())
    args.route_canary_transaction_owner_address = bytes.fromhex("41" + "99" * 20)

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "signature recovered address must match" in str(exc)
    else:
        raise AssertionError(
            "full TOML accepted route canary with mismatched signature owner"
        )


def test_full_toml_rendering_rejects_bad_route_canary_signature_recovered_address():
    module = load_evidence_module()
    args = add_route_canary_transaction_metadata(sample_full_toml_args())
    args.route_canary_signature_recovered_address = bytes.fromhex("99" * 21)

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "0x41-prefixed TRON address" in str(exc)
    else:
        raise AssertionError(
            "full TOML accepted route canary with bad signature signer address"
        )


def test_full_toml_rendering_rejects_route_canary_transaction_hash_mismatch():
    module = load_evidence_module()
    args = add_route_canary_transaction_metadata(sample_full_toml_args())
    args.route_canary_evidence_hash = bytes.fromhex("e3" * 32)

    try:
        module.render_full_toml(args, bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR))
    except ValueError as exc:
        assert "does not match route canary transaction metadata" in str(exc)
    else:
        raise AssertionError("full TOML accepted mismatched route canary transaction")


def test_full_toml_rendering_rejects_route_canary_source_record_hash_replay():
    module = load_evidence_module()

    for attr_name, label in (
        (
            "expected_source_verifier_material_hash",
            "source_verifier_material_hash",
        ),
        (
            "expected_source_adapter_engine_deployment_hash",
            "source_adapter_engine_deployment_hash",
        ),
    ):
        args = sample_full_toml_args()
        args.route_canary_evidence_hash = getattr(args, attr_name)
        args.route_canary_transaction_id = None
        args.route_canary_transaction_owner_address = None
        args.route_canary_block_number = None
        args.route_canary_block_timestamp = None
        args.route_canary_log_index = None
        args.route_canary_message_id = None
        args.route_canary_call_data_sha256 = None
        args.route_canary_payload_hash = None
        args.route_canary_target_domain = None
        args.route_canary_statement_hash = None
        args.route_canary_commitment_root = None
        args.route_canary_finality_height = None
        args.route_canary_finality_block_hash = None
        args.route_canary_proof_version = None
        args.route_canary_proof_source_domain = None
        args.route_canary_used_message_proof = None
        args.route_canary_raw_data_owner_matches_transaction = None
        args.route_canary_signature_sha256 = None
        args.route_canary_signature_recovered_address = None
        args.route_canary_signature_recovers_to_owner = None
        try:
            module._route_canary_evidence_hash(
                args,
                route_allowlist_hash=bytes.fromhex(TRON_ROUTE_ALLOWLIST_HASH_VECTOR),
                destination_binding_hash=bytes.fromhex(TRON_DESTINATION_BINDING_VECTOR),
                source_verifier_material_hash=bytes.fromhex(
                    TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
                ),
                source_adapter_engine_deployment_hash=bytes.fromhex(
                    TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
                ),
            )
        except ValueError as exc:
            assert label in str(exc)
        else:
            raise AssertionError(
                f"full TOML accepted route canary replay of {label}"
            )


def test_cli_expected_config_hash_check_accepts_matching_value(capsys):
    module = load_evidence_module()

    result = module.main(
        [
            "--bridge-address",
            "0x1111111111111111111111111111111111111111",
            "--owner-address",
            "0x2222222222222222222222222222222222222222",
            "--network-id",
            "0x" + "33" * 32,
            "--expected-config-hash",
            "0x" + TRON_SOURCE_CONFIG_VECTOR,
        ]
    )

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert output["expected_config_hash_matches"] is True
    assert output["source_bridge_network_id"] == "0x" + "33" * 32
    assert output["source_bridge_owner_address"] == "0x" + "22" * 20
    assert (
        output["adapter_verifier_vk_hash"]
        == "0x" + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
    )


def test_cli_json_emits_source_event_call_data(capsys):
    module = load_evidence_module()

    result = module.main(
        [
            "--bridge-address",
            "0x1111111111111111111111111111111111111111",
            "--owner-address",
            "0x2222222222222222222222222222222222222222",
            "--network-id",
            "0x" + "33" * 32,
            "--source-event-digest",
            "0x" + TRON_SOURCE_EVENT_DIGEST_VECTOR,
        ]
    )

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert output["source_event_digest"] == "0x" + TRON_SOURCE_EVENT_DIGEST_VECTOR
    assert output["source_event_call_data"] == (
        "0x" + TRON_SOURCE_EVENT_CALL_DATA_VECTOR
    )
    source_event_call = output["source_event_call"]
    expected_bridge = module.tron_base58check_from_address20(
        bytes.fromhex("11" * 20),
        label="bridge_address",
    )
    expected_owner = module.tron_base58check_from_address20(
        bytes.fromhex("22" * 20),
        label="owner_address",
    )
    assert source_event_call["source_bridge_address"] == expected_bridge
    assert source_event_call["source_bridge_emitter_address"] == "0x" + "11" * 20
    assert source_event_call["source_bridge_owner_address"] == "0x" + "22" * 20
    assert source_event_call["source_bridge_owner_base58"] == expected_owner
    assert source_event_call["source_event_call_data"] == (
        "0x" + TRON_SOURCE_EVENT_CALL_DATA_VECTOR
    )
    assert source_event_call["submitted_source_events_checked"] is False
    assert source_event_call["transaction_required"] is True
    assert source_event_call["trigger_request"] == {
        "endpoint": "wallet/triggersmartcontract",
        "owner_address": expected_owner,
        "contract_address": expected_bridge,
        "function_selector": "submitSccpSourceEvent(uint32,uint32,bytes32)",
        "parameter": TRON_SOURCE_EVENT_CALL_DATA_VECTOR[8:],
        "visible": True,
        "call_value": 0,
    }


def test_cli_rejects_source_event_call_data_in_toml_mode():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--source-event-digest",
                "0x" + TRON_SOURCE_EVENT_DIGEST_VECTOR,
                "--toml",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("source-event calldata rendered in TOML mode")


def test_cli_rejects_adapter_verifier_vk_hash_mismatch():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--adapter-verifier-vk-hash",
                "0x" + "99" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched adapter verifier vk hash was accepted")


def test_cli_json_emits_source_record_hashes_when_material_is_complete(capsys):
    module = load_evidence_module()

    result = module.main(
        [
            "--bridge-address",
            "0x1111111111111111111111111111111111111111",
            "--owner-address",
            "0x2222222222222222222222222222222222222222",
            "--network-id",
            "0x" + "33" * 32,
            "--expected-config-hash",
            "0x" + TRON_SOURCE_CONFIG_VECTOR,
            "--source-trust-anchor-hash",
            "0x" + "44" * 32,
            "--consensus-verifier-hash",
            "0x" + "55" * 32,
            "--message-inclusion-verifier-hash",
            "0x" + "66" * 32,
            "--source-bridge-emitter-code-hash",
            "0x" + "77" * 32,
            "--finality-policy-hash",
            "0x" + "88" * 32,
            "--deployment-receipt-hash",
            "0x" + "aa" * 32,
            "--expected-source-verifier-material-hash",
            "0x" + TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
            "--expected-source-adapter-engine-deployment-hash",
            "0x" + TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR,
            "--expected-tron-dpos-source-gate-hash",
            "0x" + TRON_DPOS_SOURCE_GATE_HASH_VECTOR,
        ]
    )

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert (
        output["source_verifier_material_hash"]
        == "0x" + TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    assert (
        output["source_adapter_engine_deployment_hash"]
        == "0x" + TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR
    )
    assert (
        output["tron_dpos_source_gate_hash"]
        == "0x" + TRON_DPOS_SOURCE_GATE_HASH_VECTOR
    )
    assert output["expected_source_verifier_material_hash_matches"] is True
    assert (
        output["expected_source_adapter_engine_deployment_hash_matches"] is True
    )
    assert output["expected_tron_dpos_source_gate_hash_matches"] is True
    assert output["toml_ready"] is True


def test_cli_json_marks_unpinned_source_records_not_toml_ready(capsys):
    module = load_evidence_module()

    result = module.main(
        [
            "--bridge-address",
            "0x1111111111111111111111111111111111111111",
            "--owner-address",
            "0x2222222222222222222222222222222222222222",
            "--network-id",
            "0x" + "33" * 32,
            "--expected-config-hash",
            "0x" + TRON_SOURCE_CONFIG_VECTOR,
            "--source-trust-anchor-hash",
            "0x" + "44" * 32,
            "--consensus-verifier-hash",
            "0x" + "55" * 32,
            "--message-inclusion-verifier-hash",
            "0x" + "66" * 32,
            "--source-bridge-emitter-code-hash",
            "0x" + "77" * 32,
            "--finality-policy-hash",
            "0x" + "88" * 32,
            "--deployment-receipt-hash",
            "0x" + "aa" * 32,
        ]
    )

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert (
        output["source_verifier_material_hash"]
        == "0x" + TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR
    )
    assert (
        output["tron_dpos_source_gate_hash"]
        == "0x" + TRON_DPOS_SOURCE_GATE_HASH_VECTOR
    )
    assert output["expected_source_verifier_material_hash_matches"] is False
    assert (
        output["expected_source_adapter_engine_deployment_hash_matches"] is False
    )
    assert output["expected_tron_dpos_source_gate_hash_matches"] is False
    assert output["toml_ready"] is False


def test_cli_rejects_expected_source_record_hash_mismatch():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-config-hash",
                "0x" + TRON_SOURCE_CONFIG_VECTOR,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
                "--expected-source-verifier-material-hash",
                "0x" + "99" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched TRON expected material hash was accepted")


def test_cli_rejects_expected_source_deployment_record_hash_mismatch():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-config-hash",
                "0x" + TRON_SOURCE_CONFIG_VECTOR,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
                "--expected-source-adapter-engine-deployment-hash",
                "0x" + "99" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError(
            "mismatched TRON expected source deployment hash was accepted"
        )


def test_cli_rejects_expected_tron_dpos_source_gate_hash_mismatch():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-config-hash",
                "0x" + TRON_SOURCE_CONFIG_VECTOR,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
                "--expected-tron-dpos-source-gate-hash",
                "0x" + "99" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched TRON DPoS source gate hash was accepted")


def test_cli_expected_source_record_hash_requires_complete_material():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-source-verifier-material-hash",
                "0x" + TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("expected source record hash accepted missing material")


def test_cli_expected_config_hash_check_rejects_mismatch():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-config-hash",
                "0x" + "44" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched expected config hash was accepted")


def test_cli_expected_destination_binding_hash_check_accepts_matching_value(capsys):
    module = load_evidence_module()

    result = module.main(
        [
            "--bridge-address",
            "0x1111111111111111111111111111111111111111",
            "--owner-address",
            "0x2222222222222222222222222222222222222222",
            "--network-id",
            "0x" + "33" * 32,
            "--destination-verifier-address",
            "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            "--destination-verifier-code-hash",
            "0x" + "bb" * 32,
            "--destination-verifier-key-hash",
            "0x" + "cc" * 32,
            "--expected-destination-binding-hash",
            "0x" + TRON_DESTINATION_BINDING_VECTOR,
        ]
    )

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert output["destination_source_domain"] == 0
    assert output["destination_target_domain"] == 5
    assert output["destination_verifier_address"] == "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"
    assert output["destination_binding_key"] == TRON_DESTINATION_BINDING_KEY_VECTOR
    assert output["destination_binding_hash"] == "0x" + TRON_DESTINATION_BINDING_VECTOR
    assert output["expected_destination_binding_hash_matches"] is True
    assert output["full_toml_ready"] is False


def test_cli_json_marks_unpinned_destination_not_full_toml_ready(capsys):
    module = load_evidence_module()

    result = module.main(
        [
            "--bridge-address",
            "0x1111111111111111111111111111111111111111",
            "--owner-address",
            "0x2222222222222222222222222222222222222222",
            "--network-id",
            "0x" + "33" * 32,
            "--destination-verifier-address",
            "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            "--destination-verifier-code-hash",
            "0x" + "bb" * 32,
            "--destination-verifier-key-hash",
            "0x" + "cc" * 32,
        ]
    )

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert output["destination_binding_hash"] == "0x" + TRON_DESTINATION_BINDING_VECTOR
    assert output["expected_destination_binding_hash_matches"] is False
    assert output["full_toml_ready"] is False
    assert "route_allowlist_hash" not in output


def test_cli_route_allowlist_hash_check_accepts_canonical_value(capsys):
    module = load_evidence_module()

    result = module.main(
        [
            "--bridge-address",
            "0x1111111111111111111111111111111111111111",
            "--owner-address",
            "0x2222222222222222222222222222222222222222",
            "--network-id",
            "0x" + "33" * 32,
            "--source-trust-anchor-hash",
            "0x" + "44" * 32,
            "--consensus-verifier-hash",
            "0x" + "55" * 32,
            "--message-inclusion-verifier-hash",
            "0x" + "66" * 32,
            "--source-bridge-emitter-code-hash",
            "0x" + "77" * 32,
            "--finality-policy-hash",
            "0x" + "88" * 32,
            "--deployment-receipt-hash",
            "0x" + "aa" * 32,
            "--destination-verifier-address",
            "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            "--destination-verifier-code-hash",
            "0x" + "bb" * 32,
            "--destination-verifier-key-hash",
            "0x" + "cc" * 32,
            "--expected-destination-binding-hash",
            "0x" + TRON_DESTINATION_BINDING_VECTOR,
            "--route-allowlist-hash",
            "0x" + TRON_ROUTE_ALLOWLIST_HASH_VECTOR,
        ]
    )

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert output["route_allowlist_hash"] == "0x" + TRON_ROUTE_ALLOWLIST_HASH_VECTOR
    assert (
        output["expected_route_allowlist_hash"]
        == "0x" + TRON_ROUTE_ALLOWLIST_HASH_VECTOR
    )
    assert output["expected_route_allowlist_hash_matches"] is True
    assert output["full_toml_ready"] is False


def test_cli_json_marks_full_rollout_without_canary_not_full_toml_ready(capsys):
    module = load_evidence_module()

    result = module.main(sample_full_toml_cli_args(include_route_canary=False))

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert output["toml_ready"] is True
    assert output["expected_destination_binding_hash_matches"] is True
    assert output["expected_route_allowlist_hash_matches"] is True
    assert output["full_toml_ready"] is False
    assert "route_canary" not in output


def test_cli_json_marks_hash_only_route_canary_not_full_toml_ready(capsys):
    module = load_evidence_module()

    result = module.main(sample_full_toml_cli_args())

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert output["toml_ready"] is True
    assert output["expected_destination_binding_hash_matches"] is True
    assert output["expected_route_allowlist_hash_matches"] is True
    assert output["full_toml_ready"] is False
    assert output["route_canary"]["status"] == "passed"
    assert output["route_canary"]["evidence_hash"] == (
        "0x" + TRON_ROUTE_CANARY_EVIDENCE_HASH
    )
    assert output["route_canary"]["transaction_owner_address"] == (
        "0x" + TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS
    )
    assert output["route_canary"]["raw_data_owner_matches_transaction"] is True
    assert output["route_canary"]["signature_sha256"] == (
        "0x" + TRON_ROUTE_CANARY_SIGNATURE_SHA256
    )
    assert output["route_canary"]["signature_recovered_address"] == (
        "0x" + TRON_ROUTE_CANARY_SIGNATURE_RECOVERED_ADDRESS
    )
    assert output["route_canary"]["signature_recovers_to_owner"] is True


def test_cli_json_marks_full_rollout_ready_with_route_canary(capsys):
    module = load_evidence_module()
    cli_args, expected = sample_full_toml_cli_args_with_runtime(module)

    result = module.main(cli_args)

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert output["toml_ready"] is True
    assert output["source_bridge_runtime_bytecode_hex"] == (
        "0x" + TRON_SOURCE_RUNTIME_BYTECODE
    )
    assert output["destination_verifier_runtime_bytecode_hex"] == (
        "0x" + TRON_DESTINATION_RUNTIME_BYTECODE
    )
    assert output["expected_destination_binding_hash_matches"] is True
    assert output["expected_route_allowlist_hash_matches"] is True
    assert output["full_toml_ready"] is True
    assert output["route_canary"]["status"] == "passed"
    assert output["route_canary"]["evidence_hash"] == (
        "0x" + expected.route_canary_evidence_hash.hex()
    )
    assert output["route_canary"]["transaction_owner_address"] == (
        "0x" + TRON_ROUTE_CANARY_TRANSACTION_OWNER_ADDRESS
    )
    assert output["route_canary"]["raw_data_owner_matches_transaction"] is True
    assert output["route_canary"]["signature_recovers_to_owner"] is True


def test_cli_route_allowlist_hash_requires_expected_destination_binding_pin():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
                "--destination-verifier-address",
                "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
                "--destination-verifier-code-hash",
                "0x" + "bb" * 32,
                "--destination-verifier-key-hash",
                "0x" + "cc" * 32,
                "--route-allowlist-hash",
                "0x" + TRON_ROUTE_ALLOWLIST_HASH_VECTOR,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("route allowlist hash accepted without binding pin")


def test_cli_expected_destination_binding_hash_check_rejects_mismatch():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--destination-verifier-address",
                "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
                "--destination-verifier-code-hash",
                "0x" + "bb" * 32,
                "--destination-verifier-key-hash",
                "0x" + "cc" * 32,
                "--expected-destination-binding-hash",
                "0x" + "44" * 32,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched destination binding hash was accepted")


def test_cli_expected_destination_binding_hash_requires_material():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-destination-binding-hash",
                "0x" + TRON_DESTINATION_BINDING_VECTOR,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("destination binding hash accepted missing material")


def test_cli_route_allowlist_hash_requires_destination_material():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--route-allowlist-hash",
                "0x" + TRON_ROUTE_ALLOWLIST_HASH_VECTOR,
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("route allowlist hash accepted missing destination material")


def test_cli_destination_binding_rejects_non_tron_target_domain():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--destination-verifier-address",
                "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
                "--destination-verifier-code-hash",
                "0x" + "bb" * 32,
                "--destination-verifier-key-hash",
                "0x" + "cc" * 32,
                "--destination-target-domain",
                "0",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("destination binding accepted non-TRON target domain")


def test_cli_toml_requires_expected_config_hash():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--adapter-verifier-vk-hash",
                "0x" + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
                "--toml",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("production TOML accepted missing expected config hash")


def test_cli_toml_requires_expected_source_record_hashes():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-config-hash",
                "0x" + TRON_SOURCE_CONFIG_VECTOR,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--adapter-verifier-vk-hash",
                "0x" + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
                "--toml",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("production TOML accepted unpinned source records")


def test_cli_toml_requires_expected_tron_dpos_source_gate_hash():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-config-hash",
                "0x" + TRON_SOURCE_CONFIG_VECTOR,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--adapter-verifier-vk-hash",
                "0x" + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
                "--expected-source-verifier-material-hash",
                "0x" + TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
                "--expected-source-adapter-engine-deployment-hash",
                "0x" + TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR,
                "--toml",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("production TOML accepted unpinned TRON source gate")


def test_cli_rejects_ambiguous_output_modes():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--toml",
                "--full-toml",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("ambiguous TRON output modes were accepted")


def test_cli_toml_derives_source_bridge_code_hash_from_runtime_bytecode(capsys):
    module = load_evidence_module()
    runtime_bytecode = bytes.fromhex("6001600055")
    derived_code_hash = module.runtime_bytecode_hash(runtime_bytecode).hex()
    expected_args = sample_full_toml_args()
    expected_args.source_bridge_emitter_code_hash = bytes.fromhex(derived_code_hash)
    expected_material_hash = module.tron_source_verifier_material_record_hash(
        expected_args,
        bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR),
    )
    expected_deployment_hash = (
        module.tron_source_adapter_engine_deployment_record_hash(
            expected_args,
            bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR),
        )
    )
    expected_gate_hash = module.tron_dpos_source_gate_hash(
        expected_args,
        bytes.fromhex(TRON_SOURCE_CONFIG_VECTOR),
    )

    result = module.main(
        [
            "--bridge-address",
            "0x1111111111111111111111111111111111111111",
            "--owner-address",
            "0x2222222222222222222222222222222222222222",
            "--network-id",
            "0x" + "33" * 32,
            "--expected-config-hash",
            "0x" + TRON_SOURCE_CONFIG_VECTOR,
            "--source-trust-anchor-hash",
            "0x" + "44" * 32,
            "--consensus-verifier-hash",
            "0x" + "55" * 32,
            "--message-inclusion-verifier-hash",
            "0x" + "66" * 32,
            "--source-bridge-runtime-bytecode-hex",
            "0x" + runtime_bytecode.hex(),
            "--finality-policy-hash",
            "0x" + "88" * 32,
            "--deployment-receipt-hash",
            "0x" + "aa" * 32,
            "--expected-source-verifier-material-hash",
            "0x" + expected_material_hash.hex(),
            "--expected-source-adapter-engine-deployment-hash",
            "0x" + expected_deployment_hash.hex(),
            "--expected-tron-dpos-source-gate-hash",
            "0x" + expected_gate_hash.hex(),
            "--toml",
        ]
    )

    assert result == 0
    output = capsys.readouterr().out
    assert f'source_bridge_emitter_code_hash = "0x{derived_code_hash}"' in output
    assert (
        '# sccp_tron_source_bridge_runtime_bytecode_hex = "0x'
        + runtime_bytecode.hex()
        + '"'
        in output
    )
    assert (
        'adapter_verifier_vk_hash = "0x'
        + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        + '"'
        in output
    )


def test_cli_rejects_source_bridge_code_hash_mismatch():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--source-bridge-runtime-bytecode-hex",
                "0x6001600055",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("mismatched source bridge code hash was accepted")


def test_cli_rejects_padded_inline_runtime_bytecode():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--source-bridge-runtime-bytecode-hex",
                " 0x6001600055",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("padded inline source bridge runtime bytecode was accepted")


def test_cli_destination_binding_derives_verifier_code_hash_from_runtime_bytecode_file(
    capsys,
    tmp_path,
):
    module = load_evidence_module()
    runtime_bytecode = bytes.fromhex("6002600055")
    bytecode_file = tmp_path / "tron_verifier_runtime.hex"
    bytecode_file.write_text("0x" + runtime_bytecode.hex() + "\n", encoding="utf-8")
    verifier_code_hash = module.runtime_bytecode_hash(runtime_bytecode)
    destination_binding_hash = module.tron_destination_binding_hash(
        network_id=bytes.fromhex("33" * 32),
        source_domain=0,
        target_domain=5,
        verifier_address="TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
        verifier_code_hash=verifier_code_hash,
        verifier_key_hash=bytes.fromhex("cc" * 32),
    )

    result = module.main(
        [
            "--bridge-address",
            "0x1111111111111111111111111111111111111111",
            "--owner-address",
            "0x2222222222222222222222222222222222222222",
            "--network-id",
            "0x" + "33" * 32,
            "--destination-verifier-address",
            "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
            "--destination-verifier-runtime-bytecode-file",
            str(bytecode_file),
            "--destination-verifier-key-hash",
            "0x" + "cc" * 32,
            "--expected-destination-binding-hash",
            "0x" + destination_binding_hash.hex(),
        ]
    )

    assert result == 0
    output = json.loads(capsys.readouterr().out)
    assert output["destination_binding_hash"] == "0x" + destination_binding_hash.hex()
    assert output["destination_verifier_runtime_bytecode_hex"] == (
        "0x" + runtime_bytecode.hex()
    )


def test_cli_rejects_unsupported_target_domain():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--target-domain",
                "99",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("unsupported target domain was accepted")


def test_cli_rejects_noncanonical_decimal_domains():
    module = load_evidence_module()

    for option, value in [
        ("--source-domain", "05"),
        ("--target-domain", "00"),
        ("--destination-source-domain", "+0"),
        ("--destination-target-domain", "0x5"),
    ]:
        try:
            module.main(
                [
                    "--bridge-address",
                    "0x1111111111111111111111111111111111111111",
                    "--owner-address",
                    "0x2222222222222222222222222222222222222222",
                    "--network-id",
                    "0x" + "33" * 32,
                    option,
                    value,
                ]
            )
        except SystemExit as exc:
            assert exc.code == 2
        else:
            raise AssertionError(f"{option} accepted non-canonical value {value!r}")


def test_toml_rendering_rejects_supported_non_sora_target_domain():
    module = load_evidence_module()
    args = SimpleNamespace(source_domain=5, target_domain=1)

    try:
        module.render_toml(args, bytes.fromhex("11" * 32))
    except ValueError as exc:
        assert "TRON -> SORA" in str(exc)
    else:
        raise AssertionError("TRON production TOML accepted a non-SORA target")

    try:
        module.render_full_toml(args, bytes.fromhex("11" * 32))
    except ValueError as exc:
        assert "TRON -> SORA" in str(exc)
    else:
        raise AssertionError("TRON full production TOML accepted a non-SORA target")

    args.target_domain = False
    try:
        module.render_toml(args, bytes.fromhex("11" * 32))
    except ValueError as exc:
        assert "target_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("TRON production TOML accepted a boolean target")

    args = SimpleNamespace(
        source_domain=5,
        target_domain=0,
        destination_source_domain=False,
        destination_target_domain=5,
        destination_proof_family="stark-fri-v1",
    )
    try:
        module.render_full_toml(args, bytes.fromhex("11" * 32))
    except ValueError as exc:
        assert "destination_source_domain must be an exact u32" in str(exc)
    else:
        raise AssertionError("TRON full production TOML accepted a boolean destination source")


def test_cli_full_toml_requires_destination_and_route_material():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-config-hash",
                "0x" + TRON_SOURCE_CONFIG_VECTOR,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--adapter-verifier-vk-hash",
                "0x" + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
                "--expected-source-verifier-material-hash",
                "0x" + TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
                "--expected-source-adapter-engine-deployment-hash",
                "0x" + TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR,
                "--expected-tron-dpos-source-gate-hash",
                "0x" + TRON_DPOS_SOURCE_GATE_HASH_VECTOR,
                "--full-toml",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("full TOML accepted missing destination material")


def test_cli_full_toml_requires_expected_source_record_hashes():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-config-hash",
                "0x" + TRON_SOURCE_CONFIG_VECTOR,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--adapter-verifier-vk-hash",
                "0x" + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
                "--destination-verifier-address",
                "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
                "--destination-verifier-code-hash",
                "0x" + "bb" * 32,
                "--destination-verifier-key-hash",
                "0x" + "cc" * 32,
                "--expected-destination-binding-hash",
                "0x" + TRON_DESTINATION_BINDING_VECTOR,
                "--route-allowlist-hash",
                "0x" + TRON_ROUTE_ALLOWLIST_HASH_VECTOR,
                "--full-toml",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("full TOML accepted missing source record hashes")


def test_cli_full_toml_requires_expected_destination_binding_hash():
    module = load_evidence_module()

    try:
        module.main(
            [
                "--bridge-address",
                "0x1111111111111111111111111111111111111111",
                "--owner-address",
                "0x2222222222222222222222222222222222222222",
                "--network-id",
                "0x" + "33" * 32,
                "--expected-config-hash",
                "0x" + TRON_SOURCE_CONFIG_VECTOR,
                "--source-trust-anchor-hash",
                "0x" + "44" * 32,
                "--consensus-verifier-hash",
                "0x" + "55" * 32,
                "--message-inclusion-verifier-hash",
                "0x" + "66" * 32,
                "--source-bridge-emitter-code-hash",
                "0x" + "77" * 32,
                "--finality-policy-hash",
                "0x" + "88" * 32,
                "--adapter-verifier-vk-hash",
                "0x" + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR,
                "--deployment-receipt-hash",
                "0x" + "aa" * 32,
                "--expected-source-verifier-material-hash",
                "0x" + TRON_SOURCE_VERIFIER_MATERIAL_HASH_VECTOR,
                "--expected-source-adapter-engine-deployment-hash",
                "0x" + TRON_SOURCE_ADAPTER_ENGINE_DEPLOYMENT_HASH_VECTOR,
                "--expected-tron-dpos-source-gate-hash",
                "0x" + TRON_DPOS_SOURCE_GATE_HASH_VECTOR,
                "--destination-verifier-address",
                "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8",
                "--destination-verifier-code-hash",
                "0x" + "bb" * 32,
                "--destination-verifier-key-hash",
                "0x" + "cc" * 32,
                "--route-allowlist-hash",
                "0x" + TRON_ROUTE_ALLOWLIST_HASH_VECTOR,
                "--full-toml",
            ]
        )
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("full TOML accepted missing destination binding hash")


def test_cli_full_toml_requires_runtime_bytecode_preimages(capsys):
    module = load_evidence_module()

    try:
        module.main([*sample_full_toml_cli_args(), "--full-toml"])
    except SystemExit as exc:
        assert exc.code == 2
    else:
        raise AssertionError("full TOML accepted hash-only runtime metadata")

    stderr = capsys.readouterr().err
    assert "deployed runtime bytecode preimages" in stderr
    assert "source-bridge-runtime-bytecode" in stderr
    assert "destination-verifier-runtime-bytecode" in stderr


def test_cli_full_toml_emits_all_production_records(capsys):
    module = load_evidence_module()
    cli_args, expected = sample_full_toml_cli_args_with_runtime(module)

    result = module.main([*cli_args, "--full-toml"])

    assert result == 0
    output = capsys.readouterr().out
    assert "[[zk.sccp_source_verifier_materials]]" in output
    assert "[[zk.sccp_source_adapter_engine_deployments]]" in output
    assert (
        '# sccp_tron_source_verifier_material_hash = "0x'
        + expected.expected_source_verifier_material_hash.hex()
        + '"'
        in output
    )
    assert '# sccp_tron_source_bridge_address = "0x' + "11" * 20 + '"' in output
    assert (
        '# sccp_tron_source_bridge_runtime_code_hash = "0x'
        + expected.source_bridge_emitter_code_hash.hex()
        + '"'
        in output
    )
    assert (
        '# sccp_tron_source_bridge_runtime_bytecode_hex = "0x'
        + TRON_SOURCE_RUNTIME_BYTECODE
        + '"'
        in output
    )
    assert (
        '# sccp_tron_source_bridge_config_hash = "0x'
        + TRON_SOURCE_CONFIG_VECTOR
        + '"'
        in output
    )
    assert (
        '# sccp_tron_source_adapter_engine_deployment_hash = "0x'
        + expected.expected_source_adapter_engine_deployment_hash.hex()
        + '"'
        in output
    )
    assert (
        '# sccp_tron_dpos_source_gate_hash = "0x'
        + expected.expected_tron_dpos_source_gate_hash.hex()
        + '"'
        in output
    )
    assert (
        'tron_dpos_source_gate_hash = "0x'
        + expected.expected_tron_dpos_source_gate_hash.hex()
        + '"'
        in output
    )
    assert "[[zk.sccp_destination_rollouts]]" in output
    assert "[[zk.sccp_route_allowlists]]" in output
    assert (
        '# sccp_tron_destination_verifier_address = "TJRabPrwbZy45sbavfcjinPJC18kjpRTv8"'
        in output
    )
    assert (
        '# sccp_tron_destination_verifier_runtime_code_hash = "0x'
        + expected.destination_verifier_code_hash.hex()
        + '"'
        in output
    )
    assert (
        '# sccp_tron_destination_verifier_runtime_bytecode_hex = "0x'
        + TRON_DESTINATION_RUNTIME_BYTECODE
        + '"'
        in output
    )
    assert (
        '# sccp_tron_destination_verifier_key_hash = "0x'
        + "cc" * 32
        + '"'
        in output
    )
    assert "# sccp_tron_destination_verifier_backend_hash" in output
    assert "# sccp_tron_destination_proof_family_hash" in output
    assert '# sccp_route_canary_status = "passed"' in output
    assert "# sccp_tron_route_canary_transaction_id" in output
    assert "tron_route_canary_transaction_id" in output
    assert "# sccp_tron_route_canary_transaction_owner_address" in output
    assert "tron_route_canary_transaction_owner_address" in output
    assert "tron_route_canary_used_message_proof = true" in output
    assert "tron_route_canary_raw_data_owner_matches_transaction = true" in output
    assert output.count("# sccp_tron_destination_binding_hash = ") == 1
    assert (
        output.count(
            'verifier_code_hash = "0x'
            + expected.destination_verifier_code_hash.hex()
            + '"'
        )
        == 1
    )
    assert (
        'adapter_verifier_vk_hash = "0x'
        + TRON_SOURCE_ADAPTER_VERIFIER_VK_HASH_VECTOR
        + '"'
        in output
    )
