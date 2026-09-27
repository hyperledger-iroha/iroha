"""Unit tests for the SCCP v1 TON toolchain, vector and StateInit tooling.

Covers `scripts/ton_sccp_builder.py` (Acton pinning and resolution,
Keccak-256, secp256k1, TON cells and BoCs, §3.7 rosters, §5.3.1 canonical
minter data, the Tolk test-vector renderer, the v1 contract source set and
its SECURITY.md) and
`scripts/generate_ton_sccp_stateinit_golden.py` (fixture consistency from
code hashes and depths alone, script-output parsing and cross-checking).
None of these tests needs Acton or network access.
"""

from __future__ import annotations

import hashlib
import io
import json
import os
import stat
import subprocess
import sys
import tarfile
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

import generate_ton_sccp_stateinit_golden as golden  # noqa: E402
import ton_sccp_builder as builder  # noqa: E402


class OpaqueCell:
    """A child cell known only by its representation hash and depth."""

    def __init__(self, hash_hex: str, depth: int) -> None:
        self.hash = bytes.fromhex(hash_hex)
        self.depth = depth


# ---------------------------------------------------------------------------
# Toolchain pinning.


def test_acton_pins_cover_native_hosts_and_are_sha256() -> None:
    assert builder.ACTON_VERSION == "1.2.0"
    assert builder.TOLK_VERSION == "1.4.2"
    assert builder.ACTON_REPORTED_VERSION.startswith("acton 1.2.0 ")
    assert builder.ACTON_RELEASE_URL.startswith("https://github.com/ton-blockchain/acton/releases/download/v1.2.0/")
    for host in (("Darwin", "arm64"), ("Darwin", "x86_64"), ("Linux", "aarch64"), ("Linux", "x86_64")):
        name, digest = builder.ACTON_ARCHIVES[host]
        assert name.startswith("acton-") and name.endswith(".tar.gz")
        assert len(digest) == 64 and int(digest, 16) >= 0
    assert builder.ACTON_ARCHIVES[("Darwin", "arm64")][0] == "acton-aarch64-apple-darwin.tar.gz"


def test_host_archive_rejects_unknown_hosts(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(builder.platform, "system", lambda: "Plan9")
    monkeypatch.setattr(builder.platform, "machine", lambda: "mips")
    with pytest.raises(builder.TonBuilderError, match="no pinned Acton"):
        builder.host_archive()


def _tar(members: dict) -> bytes:
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w:gz") as archive:
        for name, data in members.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            archive.addfile(info, io.BytesIO(data))
    return buffer.getvalue()


def test_extract_acton_takes_only_the_single_executable(tmp_path: Path) -> None:
    destination = tmp_path / "bin" / "acton"
    builder.extract_acton(_tar({"./acton": b"#!/bin/sh\necho hi\n"}), destination)
    assert destination.read_bytes() == b"#!/bin/sh\necho hi\n"
    assert destination.stat().st_mode & stat.S_IXUSR
    with pytest.raises(builder.TonBuilderError, match="unexpected Acton archive layout"):
        builder.extract_acton(_tar({"./acton": b"x", "./evil": b"y"}), tmp_path / "other")
    with pytest.raises(builder.TonBuilderError, match="unexpected Acton archive layout"):
        builder.extract_acton(_tar({"../acton": b"x"}), tmp_path / "third")


def _fake_acton(path: Path, version: str) -> Path:
    path.write_text(f"#!/bin/sh\necho '{version}'\n")
    path.chmod(0o755)
    return path


def test_explicit_acton_must_report_the_pinned_version(tmp_path: Path) -> None:
    good = _fake_acton(tmp_path / "good", builder.ACTON_REPORTED_VERSION)
    assert builder.resolve_acton(str(good)) == good
    bad = _fake_acton(tmp_path / "bad", "acton 1.1.0 (9cf4d1f 2026-05-22)")
    with pytest.raises(builder.TonBuilderError, match="expected"):
        builder.resolve_acton(str(bad))
    with pytest.raises(builder.TonBuilderError, match="absolute executable"):
        builder.resolve_acton("relative/acton")


def test_offline_resolution_never_downloads(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(builder, "TOOLCHAIN_DIR", tmp_path / "empty")

    def refuse(*_args, **_kwargs):  # pragma: no cover - must not be called
        raise AssertionError("network used")

    monkeypatch.setattr(builder.urllib.request, "urlopen", refuse)
    with pytest.raises(builder.TonBuilderError, match="--offline"):
        builder.resolve_acton(None, offline=True)


def test_download_verifies_the_archive_digest(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(builder, "TOOLCHAIN_DIR", tmp_path / "cache")
    archive = _tar({"./acton": b"#!/bin/sh\necho wrong\n"})

    class Response(io.BytesIO):
        def __enter__(self):
            return self

        def __exit__(self, *_args):
            return False

    monkeypatch.setattr(builder.urllib.request, "urlopen", lambda *_a, **_k: Response(archive))
    with pytest.raises(builder.TonBuilderError, match="SHA-256 mismatch"):
        builder.resolve_acton(None)
    # a digest-correct archive that reports the wrong version is refused too
    name, _ = builder.host_archive()
    monkeypatch.setitem(
        builder.ACTON_ARCHIVES,
        (builder.platform.system(), builder.platform.machine()),
        (name, hashlib.sha256(archive).hexdigest()),
    )
    with pytest.raises(builder.TonBuilderError, match="downloaded Acton reports"):
        builder.resolve_acton(None)


def test_tolk_stdlib_check(tmp_path: Path) -> None:
    acton_dir = tmp_path / ".acton" / "tolk-stdlib"
    acton_dir.mkdir(parents=True)
    (tmp_path / ".acton" / ".version").write_text("1.2.0")
    (acton_dir / "common.tolk").write_text("// Standard library\n// x\n// y\ntolk 1.4.2\n")
    builder.check_tolk_stdlib(tmp_path)
    (acton_dir / "common.tolk").write_text("tolk 1.4.1\n")
    with pytest.raises(builder.TonBuilderError, match="tolk 1.4.2"):
        builder.check_tolk_stdlib(tmp_path)
    (tmp_path / ".acton" / ".version").write_text("1.1.0")
    with pytest.raises(builder.TonBuilderError, match="Acton 1.2.0"):
        builder.check_tolk_stdlib(tmp_path)


def test_no_docker_corridor_or_environment_toggles_remain() -> None:
    python_source = (SCRIPTS / "ton_sccp_builder.py").read_text(encoding="utf-8")
    golden_source = (SCRIPTS / "generate_ton_sccp_stateinit_golden.py").read_text(encoding="utf-8")
    wrapper = (SCRIPTS / "sccp_ton_contract_build.sh").read_text(encoding="utf-8")
    for source in (python_source, golden_source, wrapper):
        for retired in ("docker run", '"docker"', "--pull=never", "production-prepare", "production-release"):
            assert retired not in source
        assert "os.environ" not in source
        assert "replay_forest" not in source
        assert "ACTON_BIN" not in source
    assert 'exec python3 "$script_dir/ton_sccp_builder.py" "$@"' in wrapper
    assert os.stat(SCRIPTS / "ton_sccp_builder.py").st_mode & 0o111


def test_acton_project_pins_the_toolchain_and_pascal_case_contracts() -> None:
    manifest = (builder.PROJECT / "Acton.toml").read_text(encoding="utf-8")
    assert 'acton = "1.2.0"' in manifest
    for name in builder.CONTRACTS:
        assert f"[contracts.{name}]" in manifest
        assert (builder.PROJECT / "contracts" / f"{name}.tolk").is_file()
        assert (builder.PROJECT / "wrappers" / f"{name}.gen.tolk").is_file()
    for retired in ("TairaXorSccpBridge", "TairaXorJettonMaster", "proof-verifier", "replay-forest"):
        assert retired not in manifest
    assert ".toolchain/" in (builder.PROJECT / ".gitignore").read_text(encoding="utf-8")


# Sources of the retired bridge/master/wallet, proof verifier and replay forest.
# TODO(ws14): the retired wrappers (`wrappers/TairaXor*.gen.tolk`), the old
# suites directly under `tests/`, `scripts/generate-stateinit-golden.tolk` and
# `fixtures/sccp/ton_stateinit_golden_v1.json` await deletion by the
# orchestrator (the fixture only after `crates/iroha_sccp/src/ton_native.rs`,
# owned by ws10/ws3A, stops including it); add them here once removed.
RETIRED_CONTRACT_SOURCES = (
    "TairaXorSccpBridge.tolk",
    "TairaXorJettonMaster.tolk",
    "TairaXorJettonWallet.tolk",
    "proof-verifier.tolk",
    "replay-forest.tolk",
    "sccp-codec.tolk",
    "constants.tolk",
    "errors.tolk",
    "jetton-utils.tolk",
    "messages.tolk",
    "storage.tolk",
)


def test_contract_sources_are_exactly_the_v1_set() -> None:
    contracts = builder.PROJECT / "contracts"
    present = sorted(path.name for path in contracts.glob("*.tolk"))
    expected = sorted(Path(path).name for path in builder.SOURCE_FILES if path.startswith("contracts/"))
    assert present == expected
    for retired in RETIRED_CONTRACT_SOURCES:
        assert not (contracts / retired).exists()


def test_security_notes_describe_the_v1_design() -> None:
    text = (builder.PROJECT / "SECURITY.md").read_text(encoding="utf-8")
    for retired in (
        "SccpDisableMinting",
        "3-of-5",
        "Ed25519",
        "forest",
        "mintingDisabled",
        "ton_stateinit_golden_v1",
        "Acton 1.1.0",
        "Tolk 1.4.1",
        "Linux/amd64",
    ):
        assert retired not in text
    for current in (
        "sccp_apply_control",
        "control_nonce",
        "raw_reserve",
        "MINTER_FLOOR",
        "BUCKET_FLOOR",
        "library",
        "fixtures/sccp/ton_stateinit_v1.json",
        "Acton 1.2.0",
        "Tolk 1.4.2",
    ):
        assert current in text


def test_cli_rejects_unknown_arguments() -> None:
    result = subprocess.run(
        [sys.executable, str(SCRIPTS / "ton_sccp_builder.py"), "--unknown"],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    assert result.returncode == 2


# ---------------------------------------------------------------------------
# Keccak-256 and secp256k1.


def test_keccak256_known_answers() -> None:
    assert builder.keccak256(b"").hex() == "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    assert builder.keccak256(b"SCCP").hex() == "d7bacbdfe367013f66397ff7242325c31126ab14c95620817c994f22f1d123ba"
    assert builder.keccak256(b"1").hex() == "c89efdaa54c0f20c7adf612882df0950f5a951637e0307cdcb4c672f298b8bc6"
    # rate boundary (135, 136, 137 bytes) agrees with pycryptodome when present
    try:
        from Crypto.Hash import keccak  # type: ignore
    except ImportError:
        keccak = None
    if keccak is not None:
        for size in (135, 136, 137, 272, 1000):
            data = bytes(range(256)) * 4
            reference = keccak.new(digest_bits=256)
            reference.update(data[:size])
            assert builder.keccak256(data[:size]) == reference.digest()


def test_spec_constants_self_check() -> None:
    builder._self_check_spec_constants()


def test_ecdsa_sign_is_low_s_and_recovers() -> None:
    private = 0x1234567890ABCDEF
    public = builder.point_mul(private)
    assert public is not None
    address = builder.eth_address(public)
    for nonce in (5, 7, 0xDEADBEEF):
        digest = builder.keccak_int(nonce.to_bytes(8, "big"))
        r, s, v = builder.ecdsa_sign(private, digest, nonce)
        assert 1 <= r < builder.SECP_N and 1 <= s <= builder.SECP_HALF_N and v in (27, 28)
        assert builder.ecdsa_recover(digest, r, s, v) == address
        assert builder.ecdsa_recover(digest + 1, r, s, v) != address


def test_eth_address_of_generator_point() -> None:
    # address of private key 1 (well-known)
    assert builder.eth_address(builder.SECP_G) == 0x7E5F4552091A69125D5DFCB7B8C2659029395BDF


# ---------------------------------------------------------------------------
# TON cells and BoCs.


def test_cell_hashes_match_ton_reference_values() -> None:
    assert builder.Cell("").hash.hex() == "96a296d224f285c67bee93c30f8a309157f0daa35dc5b87e410b78630a09cfc7"
    child = builder.Builder().uint(1, 8).end()
    parent = builder.Builder().uint(0b101, 3).ref(child).end()
    assert parent.depth == 1 and child.depth == 0
    material = bytes([1, 1]) + bytes([0b10110000]) + (0).to_bytes(2, "big") + child.hash
    assert parent.hash == hashlib.sha256(material).digest()


def test_builder_serialization_rules() -> None:
    assert builder.Builder().coins(0).bits == "0000"
    assert builder.Builder().coins(256).bits == "0010" + format(256, "016b")
    assert builder.Builder().address(0, 5).bits == "100" + "0" * 8 + format(5, "0256b")
    assert builder.Builder().int(-1, 8).bits == "11111111"
    assert builder.Builder().maybe_ref(None).bits == "0"
    with pytest.raises(builder.TonBuilderError):
        builder.Builder().uint(256, 8)
    with pytest.raises(builder.TonBuilderError):
        builder.Cell("1" * 1024)


def _serialize_boc(root: builder.Cell) -> bytes:
    order = []
    index = {}

    def visit(cell: builder.Cell) -> None:
        if cell.hash in index:
            return
        index[cell.hash] = len(order)
        order.append(cell)
        for ref in cell.refs:
            visit(ref)

    visit(root)
    body = b""
    for cell in order:
        body += cell.descriptors() + cell.data_bytes() + bytes(index[ref.hash] for ref in cell.refs)
    header = bytes.fromhex("b5ee9c72") + bytes([0x01, 0x02]) + bytes([len(order), 1, 0])
    header += len(body).to_bytes(2, "big") + bytes([0])
    return header + body


def test_parse_boc_round_trips_a_cell_tree() -> None:
    leaf = builder.Builder().uint(0xABC, 12).end()
    mid = builder.Builder().uint(1, 1).ref(leaf).end()
    root = builder.Builder().uint(0x55, 8).ref(mid).ref(leaf).end()
    parsed = builder.parse_boc(_serialize_boc(root))
    assert parsed.hash == root.hash
    assert parsed.depth == 2
    assert builder.cell_tree_size(parsed) == (3, 8 + 1 + 12)
    with pytest.raises(builder.TonBuilderError, match="serialized_boc"):
        builder.parse_boc(b"\x00" * 16)


# ---------------------------------------------------------------------------
# Rosters and canonical minter data.


def test_roster_digest_rules() -> None:
    members = [golden.golden_member(i) for i in range(1, 5)]
    digest = builder.roster_digest(golden.NETWORK_ID, 7, 1, 2, members)
    preimage = (
        b"SCCP/ROSTER/V1" + bytes.fromhex("11" * 32) + (7).to_bytes(8, "big") + (1).to_bytes(8, "big")
        + (2).to_bytes(8, "big") + bytes([4, 3]) + b"".join(m.to_bytes(20, "big") for m in members)
    )
    assert digest == builder.keccak_int(preimage)
    assert builder.roster_threshold(4) == 3 and builder.roster_threshold(31) == 21
    builder.roster_digest(golden.NETWORK_ID, 7, 1, 2, [0, 0] + members[:3])
    for bad in (
        members[:3],
        [members[1], members[0]] + members[2:],
        [members[0], 0] + members[2:],
        [members[0], members[0]] + members[2:],
        [0] * 32,
    ):
        with pytest.raises(builder.TonBuilderError):
            builder.roster_digest(golden.NETWORK_ID, 7, 1, 2, bad)
    with pytest.raises(builder.TonBuilderError):
        builder.roster_digest(golden.NETWORK_ID, 7, 2, 2, members)
    with pytest.raises(builder.TonBuilderError):
        builder.roster_digest(golden.NETWORK_ID, 0, 1, 2, members)


def test_member_chunks_are_maximal() -> None:
    members = list(range(1, 14))
    chunk = builder.member_chunks(members)
    sizes = []
    current = chunk
    while True:
        sizes.append((len(current.bits) - 1) // 160)
        if not current.refs:
            assert current.bits[-1] == "0"
            break
        assert current.bits[-1] == "1"
        current = current.refs[0]
    assert sizes == [6, 6, 1]


def test_minter_initial_data_layout() -> None:
    wallet = OpaqueCell("aa" * 32, 3)
    bucket = OpaqueCell("bb" * 32, 5)
    members = [golden.golden_member(i) for i in range(1, 5)]
    cells = builder.minter_initial_data(golden.NETWORK_ID, 1, 10**18, 7, 1, 2, members, wallet, bucket)
    root = cells["root"]
    assert len(root.bits) == 267 and root.refs == [cells["config"], cells["roster"]]
    assert set(root.bits) == {"0"}
    assert cells["config"].refs == [wallet, bucket]
    assert len(cells["roster"].bits) == 256 + 3 * 64 + 16
    si = builder.state_init(OpaqueCell("cc" * 32, 9), root)
    assert si.bits == "00110"


def test_state_init_hash_matches_tvm_formula() -> None:
    code = OpaqueCell("01" * 32, 2)
    data = builder.Builder().uint(1, 1).end()
    si = builder.state_init(code, data)
    material = bytes([2, 1]) + bytes([0b00110100]) + (2).to_bytes(2, "big") + data.depth.to_bytes(2, "big")
    material += code.hash + data.hash
    assert si.hash == hashlib.sha256(material).digest()


# ---------------------------------------------------------------------------
# Tolk test vectors.


def test_test_keys_are_sorted_and_sign_consistently() -> None:
    keys = builder.test_keys()
    assert len(keys) == builder.TEST_KEY_COUNT
    addresses = [key["address"] for key in keys]
    assert addresses == sorted(addresses) and len(set(addresses)) == len(addresses)
    digest = builder.keccak_int(b"vector")
    for key in keys[:3]:
        r, s, v = builder.ecdsa_sign(key["private"], digest, key["nonce"])
        assert r == key["r"]
        assert key["k_inverse"] * key["nonce"] % builder.SECP_N == 1
        assert builder.ecdsa_recover(digest, r, s, v) == key["address"]


def test_committed_tolk_vectors_are_current() -> None:
    assert builder.VECTORS_FILE.read_text(encoding="utf-8") == builder.render_test_vectors()


def test_merkle_helpers_follow_promote_odd() -> None:
    leaves = [builder.keccak_int(bytes([i])) for i in range(5)]
    levels = builder.merkle_levels(leaves)
    assert [len(level) for level in levels] == [5, 3, 2, 1]
    assert levels[1][2] == leaves[4]
    assert builder.merkle_path(leaves, 4) == [levels[2][0]]
    assert len(builder.merkle_path(leaves, 3)) == 3


# ---------------------------------------------------------------------------
# StateInit golden.


def _fixture() -> dict:
    return json.loads(golden.FIXTURE.read_text(encoding="utf-8"))


def test_fixture_is_canonical_json() -> None:
    fixture = _fixture()
    assert golden.render(fixture) == golden.FIXTURE.read_text(encoding="utf-8")
    assert fixture["schema"] == golden.SCHEMA
    assert fixture["toolchain"] == {"acton": "1.2.0", "tolk": "1.4.2"}
    assert [vector["label"] for vector in fixture["vectors"]] == ["n4", "n31"]


def test_fixture_reproduces_from_code_hashes_and_depths_alone() -> None:
    fixture = _fixture()
    code = {name: OpaqueCell(record["hash"], record["depth"]) for name, record in fixture["code"].items()}
    for vector, spec in zip(fixture["vectors"], golden.VECTORS):
        members = [int(member, 16) for member in vector["members"]]
        assert members == spec["members"]
        network = int(vector["taira_network_id"], 16)
        cells = builder.minter_initial_data(
            network,
            vector["route_revision"],
            int(vector["max_supply"]),
            vector["generation"],
            vector["valid_from_ms"],
            vector["valid_until_ms"],
            members,
            code["wallet"],
            code["bucket"],
        )
        assert cells["root"].hash.hex() == vector["initial_data"]["hash"]
        assert cells["root"].depth == vector["initial_data"]["depth"]
        assert cells["config"].data_hex() == vector["initial_data"]["config"]["data"]
        assert f"{builder.roster_digest(network, vector['generation'], vector['valid_from_ms'], vector['valid_until_ms'], members):064x}" == vector["roster_digest"]
        state_init = builder.state_init(code["minter"], cells["root"])
        assert state_init.hash.hex() == vector["state_init_hash"] == vector["address"]["account_id"]
        account = int(vector["address"]["account_id"], 16)
        children = vector["children"]
        assert f"{golden.wallet_address(golden.WALLET_OWNER, account, code['wallet']):064x}" == children["wallet_account_id"]
        assert f"{golden.bucket_address(account, 0, code['bucket']):064x}" == children["bucket_0_account_id"]
        assert f"{golden.bucket_address(account, 1, code['bucket']):064x}" == children["bucket_1_account_id"]
        assert vector["t"] == builder.roster_threshold(vector["n"])
    n31 = fixture["vectors"][1]
    assert len(n31["initial_data"]["members"]) == 6
    assert n31["members"][:3] == ["0" * 40] * 3


def test_script_output_parser_and_cross_check() -> None:
    fixture = _fixture()
    lines = []
    for name, record in fixture["code"].items():
        lines.append(f"sccp-stateinit code {name}_code_hash={record['hash']}")
        lines.append(f"sccp-stateinit code {name}_code_depth={record['depth']}")
    for vector in fixture["vectors"]:
        label = vector["label"]
        lines += [
            f"sccp-stateinit {label} roster_digest={vector['roster_digest']}",
            f"sccp-stateinit {label} data_hash={vector['initial_data']['hash']}",
            f"sccp-stateinit {label} data_depth={vector['initial_data']['depth']}",
            f"sccp-stateinit {label} state_init_hash={vector['state_init_hash']}",
            f"sccp-stateinit {label} workchain=0",
            f"sccp-stateinit {label} account_id={vector['address']['account_id']}",
            f"sccp-stateinit {label} wallet_of_abab={vector['children']['wallet_account_id']}",
            f"sccp-stateinit {label} bucket_0={vector['children']['bucket_0_account_id']}",
            f"sccp-stateinit {label} bucket_1={vector['children']['bucket_1_account_id']}",
        ]
    output = "noise line\n" + "\n".join(lines) + "\n"
    golden.cross_check(fixture, golden.parse_script_output(output))
    tampered = output.replace(fixture["vectors"][0]["address"]["account_id"], "00" * 32, 1)
    with pytest.raises(golden.GoldenError):
        golden.cross_check(fixture, golden.parse_script_output(tampered))
    with pytest.raises(golden.GoldenError, match="duplicate"):
        golden.parse_script_output(output + lines[0] + "\n")
    with pytest.raises(golden.GoldenError, match="keys differ"):
        golden.cross_check(fixture, golden.parse_script_output("\n".join(lines[1:])))
