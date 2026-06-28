import sys
from importlib.util import module_from_spec, spec_from_file_location
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parents[2] / "scripts"

LANE_CLI_HELPERS = (
    (
        "sccp_all_lanes_evidence.py",
        "SCCP all-lanes evidence validation failed",
    ),
    (
        "sccp_eth_source_bridge_evidence.py",
        "SCCP Ethereum source bridge evidence rendering failed",
    ),
    (
        "sccp_bsc_source_bridge_evidence.py",
        "SCCP BSC source bridge evidence rendering failed",
    ),
    (
        "sccp_evm_destination_evidence.py",
        "SCCP EVM destination evidence rendering failed",
    ),
    (
        "sccp_evm_receipt_proof_evidence.py",
        "SCCP EVM receipt proof evidence collection failed",
    ),
    (
        "sccp_evm_source_live_evidence.py",
        "SCCP EVM source live evidence collection failed",
    ),
    (
        "sccp_evm_live_evidence.py",
        "SCCP EVM live evidence collection failed",
    ),
    (
        "sccp_solana_destination_evidence.py",
        "SCCP Solana destination evidence rendering failed",
    ),
    (
        "sccp_solana_source_state_evidence.py",
        "SCCP Solana source-state evidence rendering failed",
    ),
    (
        "sccp_solana_live_evidence.py",
        "SCCP Solana live evidence collection failed",
    ),
    (
        "sccp_ton_destination_evidence.py",
        "SCCP TON destination evidence rendering failed",
    ),
    (
        "sccp_ton_source_state_evidence.py",
        "SCCP TON source-state evidence rendering failed",
    ),
    (
        "sccp_ton_live_evidence.py",
        "SCCP TON live evidence collection failed",
    ),
    (
        "sccp_tron_source_bridge_evidence.py",
        "SCCP TRON source bridge evidence rendering failed",
    ),
    (
        "sccp_tron_live_evidence.py",
        "SCCP TRON live evidence collection failed",
    ),
)

SENSITIVE_MESSAGES = (
    ("operator secret-token value", ("secret-token",)),
    ("operator secret%20key value", ("secret%20key",)),
    ("operator secret%2dtoken value", ("secret%2dtoken",)),
    ("operator secret%252dtoken value", ("secret%252dtoken",)),
    ("operator secret&amp;#45;token value", ("secret&amp;#45;token",)),
    (
        "operator \u0455\u0435\u0441r\u0435t-token value",
        ("\u0455\u0435\u0441r\u0435t-token",),
    ),
    (
        "operator %D1%95%D0%B5%D1%81r%D0%B5t-token value",
        ("%D1%95%D0%B5%D1%81r%D0%B5t-token",),
    ),
    ("operator private&#95;key value", ("private&#95;key",)),
    ("operator private&amp;#95;key value", ("private&amp;#95;key",)),
    ("operator private%252dkey value", ("private%252dkey",)),
    (
        "operator private-\u03baey value",
        ("private-\u03baey",),
    ),
    (
        "operator private-%CE%BAey value",
        ("private-%CE%BAey",),
    ),
    ("operator password value", ("password",)),
    ("operator passphrase value", ("passphrase",)),
    ("operator bearer value", ("bearer",)),
    ("operator authorization value", ("authorization",)),
    ("operator access&#45;key value", ("access&#45;key",)),
    ("operator access&amp;#45;key value", ("access&amp;#45;key",)),
    ("operator api%20key value", ("api%20key",)),
    ("operator client&#32;secret value", ("client&#32;secret",)),
    ("operator client&amp;#32;secret value", ("client&amp;#32;secret",)),
    ("operator credential value", ("credential",)),
    ("operator auth_header value", ("auth_header",)),
    ("operator mnemonic value", ("mnemonic",)),
    ("operator recovery%2dphrase value", ("recovery%2dphrase",)),
    ("operator recovery%252dphrase value", ("recovery%252dphrase",)),
    (
        "operator recovery-\u0440hrase value",
        ("recovery-\u0440hrase",),
    ),
    (
        "operator recovery-%D1%80hrase value",
        ("recovery-%D1%80hrase",),
    ),
    ("operator seed%20phrase value", ("seed%20phrase",)),
    ("operator signing%20key value", ("signing%20key",)),
    (
        "operator t\u03bfken value",
        ("t\u03bfken",),
    ),
    (
        "operator t%CE%BFken value",
        ("t%CE%BFken",),
    ),
    (
        "operator t%25CE%25BFken value",
        ("t%25CE%25BFken",),
    ),
    ("operator session value", ("session",)),
    ("operator token value", ("token",)),
    ("operator clé value", ("clé value",)),
    ("operator" + "\n" + "value", ("operator", "value")),
    ("operator" + "\t" + "value", ("operator", "value")),
    ("operator" + "\x7f" + "value", ("operator", "value")),
)

UNSAFE_MESSAGES = (
    "safe%0Acollector detail",
    "safe%E2%80%AEcollector detail",
    "safe%7Ccollector detail",
    "safe%3Ccollector detail%3E",
    "safe&#10;collector detail",
    "safe&amp;#10;collector detail",
    "safe&#124;collector detail",
    "safe&amp;#124;collector detail",
)


def load_helper(script_name):
    spec = spec_from_file_location(
        script_name.removesuffix(".py"), SCRIPT_DIR / script_name
    )
    module = module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)  # type: ignore[assignment]
    return module


def test_lane_cli_error_detail_redacts_decoded_sensitive_messages():
    for script_name, fallback in LANE_CLI_HELPERS:
        module = load_helper(script_name)
        exception_types = (
            module.argparse.ArgumentTypeError,
            OSError,
            SystemExit,
            RuntimeError,
            TypeError,
            ValueError,
        )
        for exception_type in exception_types:
            for message, leaked_markers in SENSITIVE_MESSAGES:
                detail = module._cli_error_detail(
                    exception_type(message), fallback=fallback
                )
                assert detail == fallback, (script_name, exception_type, message)
                for leaked_marker in leaked_markers:
                    assert leaked_marker not in detail


def test_lane_cli_error_detail_redacts_decoded_unsafe_messages():
    for script_name, fallback in LANE_CLI_HELPERS:
        module = load_helper(script_name)
        for message in UNSAFE_MESSAGES:
            detail = module._cli_error_detail(RuntimeError(message), fallback=fallback)
            assert detail == fallback, (script_name, message)


def test_lane_cli_error_detail_preserves_safe_runtime_error_messages():
    safe_message = "route evidence is temporarily unavailable"
    for script_name, fallback in LANE_CLI_HELPERS:
        module = load_helper(script_name)
        detail = module._cli_error_detail(RuntimeError(safe_message), fallback=fallback)
        assert detail == safe_message, script_name
