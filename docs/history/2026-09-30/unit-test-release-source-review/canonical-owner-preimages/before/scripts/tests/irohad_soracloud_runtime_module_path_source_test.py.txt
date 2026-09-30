"""Source contracts for the always-loaded SoraCloud runtime module tree."""

from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAIN_SOURCE = ROOT / "crates/irohad/src/main.rs"
RUNTIME_SOURCE = ROOT / "crates/irohad/src/soracloud_runtime.rs"
STUB_RUNTIME_SOURCE = ROOT / "crates/irohad/src/soracloud_runtime_stub.rs"
IROHAD_MANIFEST = ROOT / "crates/irohad/Cargo.toml"
READINESS_SCRIPT = ROOT / "scripts/ci/run_soracloud_production_readiness.sh"
TOKEN_AUTH_SOURCE = (
    ROOT / "crates/irohad/src/soracloud_runtime/remote_stream_token_auth.rs"
)


def test_always_loaded_runtime_uses_explicit_nested_token_auth_path() -> None:
    main = MAIN_SOURCE.read_text(encoding="utf-8")
    runtime = RUNTIME_SOURCE.read_text(encoding="utf-8")

    assert '#[path = "soracloud_runtime.rs"]\nmod soracloud_runtime;' in main
    assert (
        '#[path = "soracloud_runtime/remote_stream_token_auth.rs"]\n'
        "mod remote_stream_token_auth;"
    ) in runtime
    assert TOKEN_AUTH_SOURCE.is_file()


def test_runtime_keeps_config_binding_and_explicit_read_disambiguation() -> None:
    runtime = RUNTIME_SOURCE.read_text(encoding="utf-8")
    token_auth = TOKEN_AUTH_SOURCE.read_text(encoding="utf-8")

    assert "with_remote_stream_token_operator_from_config" in token_auth
    assert "NetworkId::from_genesis_hash(config.genesis.expected_hash)" in token_auth
    assert "std::io::Read::by_ref(&mut file)" in runtime


def test_runtime_stub_is_absent_and_readiness_asserts_the_hard_cut() -> None:
    main = MAIN_SOURCE.read_text(encoding="utf-8")
    manifest = IROHAD_MANIFEST.read_text(encoding="utf-8")
    readiness = READINESS_SCRIPT.read_text(encoding="utf-8")

    assert not STUB_RUNTIME_SOURCE.exists()
    assert "soracloud_runtime_stub" not in main
    assert "soracloud_runtime_stub" not in manifest
    assert "cargo test -p irohad --bin iroha3d stub_runtime_" not in readiness
    assert "test ! -e crates/irohad/src/soracloud_runtime_stub.rs" in readiness
    assert "! rg -n 'soracloud_runtime_stub|stub_runtime_'" in readiness
