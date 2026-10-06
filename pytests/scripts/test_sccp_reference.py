"""Tests for the independent SCCP Python reference (`scripts/sccp_reference/`, `specs/sccp.md` revision 5).

They pin the primitives against published vectors (RFC 9380, Ethereum
bls12-381-tests inputs, a captured Ethereum mainnet sync-committee aggregate,
values cross-checked against `blst`), against the Rust-generated fixtures
whose layouts revision 5 keeps (payloads, transfer leaves, the commitment tree
and the history accumulator), exercise the destination model directly, and
regenerate every fixture the reference owns to fail on drift.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from sccp_reference import bls12_381 as bls  # noqa: E402
from sccp_reference import generate  # noqa: E402
from sccp_reference.bls_sig import (  # noqa: E402
    aggregate,
    aggregate_verify,
    consensus_digest,
    fast_aggregate_verify,
    key_validate,
    keygen,
    sign,
    signature_validate,
    sk_to_pk,
    verify,
)
from sccp_reference.common import SccpError, h_iroha, rep, unhex  # noqa: E402
from sccp_reference.destination import Deployment, Destination  # noqa: E402
from sccp_reference.finality import (  # noqa: E402
    FLAG_ACTIVE,
    FLAG_ROTATION,
    FinalityHeader,
    anchor_chunk_levels,
    anchor_chunk_path,
    anchor_leaf,
    anchor_node,
    bitmap_from_indices,
    build_certificate,
    commit_preimage,
    committee_n_valid,
    inactive_header,
    parse_header,
    parse_qc_fixed,
    quorum,
    result_of_preimage,
    signers_from_indices,
)
from sccp_reference.hash_to_curve import (  # noqa: E402
    DST_SIG,
    SSWU_Z,
    expand_message_xmd,
    hash_to_g2,
    hash_to_g2_trace,
)
from sccp_reference.imported_vectors import (  # noqa: E402
    EXPAND_MESSAGE_XMD_DST,
    EXPAND_MESSAGE_XMD_SHA256,
    HASH_TO_G2_RO,
    HASH_TO_G2_RO_DST,
)
from sccp_reference.keccak import event_topic, keccak256, keccak_f1600, selector  # noqa: E402
from sccp_reference.merkle import (  # noqa: E402
    bag_peaks,
    check_block_ref,
    check_leaf,
    control_leaf,
    history_leaf,
    history_peaks,
    history_root,
    merkle_root,
    transfer_leaf,
    tree_levels,
    tree_path,
    tree_root,
)
from sccp_reference.payload import (  # noqa: E402
    BY_DOMAIN,
    ETHEREUM,
    TAIRA,
    TON,
    decode_payload,
    identity_word,
    message_id,
    payload_hash,
)
from sccp_reference.taira import Committee, TairaSim  # noqa: E402

FIXTURES = ROOT / "fixtures" / "sccp"


# -- Keccak ---------------------------------------------------------------------


def _sha3_256_with(permutation, data: bytes) -> bytes:
    padded = bytearray(data) + b"\x06"
    while len(padded) % 136:
        padded.append(0)
    padded[-1] |= 0x80
    state = [0] * 25
    for off in range(0, len(padded), 136):
        for i in range(17):
            state[i] ^= int.from_bytes(padded[off + 8 * i : off + 8 * i + 8], "little")
        permutation(state)
    return b"".join(state[i].to_bytes(8, "little") for i in range(4))


def test_keccak256_known_values():
    assert keccak256(b"").hex() == "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    assert keccak256(b"abc").hex() == "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"
    assert keccak256(b"ab", b"c") == keccak256(b"abc")


@pytest.mark.parametrize("length", [0, 1, 55, 135, 136, 137, 271, 272, 300])
def test_keccak_permutation_matches_fips202(length):
    data = bytes((7 * i + 1) % 256 for i in range(length))
    assert _sha3_256_with(keccak_f1600, data) == hashlib.sha3_256(data).digest()


def test_selector_and_event_topic():
    assert selector("submitCheckpoints((bytes,bytes,bytes,bytes,bytes)[])").hex() == "07a5e706"
    assert event_topic("SccpTransferToTaira(bytes32,address,uint64,bytes)").hex() == (
        "79ac1cc63262b80bfbf96e55b2d49d96bd82f018ce0192f7002168724c7d264b"
    )


# -- BLS12-381 arithmetic ----------------------------------------------------------


def test_generators_constants_and_order():
    assert bls.g1_is_on_curve(bls.G1_GENERATOR) and bls.g2_is_on_curve(bls.G2_GENERATOR)
    assert bls.g1_in_subgroup(bls.G1_GENERATOR) and bls.g2_in_subgroup(bls.G2_GENERATOR)
    assert bls.g1_compress(bls.G1_GENERATOR).hex().startswith("97f1d3a73197d794")
    assert bls.g2_compress(bls.G2_GENERATOR).hex().startswith("93e02b6052719f60")
    assert bls.HALF_P == 0x0D0088F51CBFF34D258DD3DB21A5D66BB23BA5C279C2895FB39869507B587B120F55FFFF58A9FFFFDCFF7FFFFFFFD555


def test_field_sqrt_and_inverses():
    for a in (2, 3, 5, 12345678901234567890):
        sq = a * a % bls.P
        assert bls.fp_sqrt(sq) in (a % bls.P, -a % bls.P)
        assert bls.fp_inv(a) * a % bls.P == 1
        x = (a, a + 1)
        assert bls.f2_sqrt(bls.f2_sqr(x)) in (x, bls.f2_neg(x))
        assert bls.f2_mul(x, bls.f2_inv(x)) == bls.F2_ONE
    assert bls.f2_sqrt(SSWU_Z) is None and not bls.f2_is_square(SSWU_Z)  # Z = −(2 + u) is a non-square
    f = bls.miller_loop(bls.G1_GENERATOR, bls.G2_GENERATOR)
    assert bls.f12_mul(f, bls.f12_inv(f)) == bls.F12_ONE


def test_scalar_multiplication_matches_repeated_addition():
    acc1, acc2 = None, None
    for k in range(1, 12):
        acc1 = bls.g1_add(acc1, bls.G1_GENERATOR)
        acc2 = bls.g2_add(acc2, bls.G2_GENERATOR)
        assert bls.g1_mul(bls.G1_GENERATOR, k) == acc1
        assert bls.g2_mul(bls.G2_GENERATOR, k) == acc2
    assert bls.g1_mul(bls.G1_GENERATOR, bls.R) is None
    assert bls.g2_mul(bls.G2_GENERATOR, -1) == bls.g2_neg(bls.G2_GENERATOR)


def test_pairing_is_bilinear_and_non_degenerate():
    e = bls.pairing(bls.G1_GENERATOR, bls.G2_GENERATOR)
    assert e != bls.F12_ONE
    assert bls.pairing(bls.g1_mul(bls.G1_GENERATOR, 3), bls.G2_GENERATOR) == bls.pairing(
        bls.G1_GENERATOR, bls.g2_mul(bls.G2_GENERATOR, 3)
    )
    assert bls.pairing_product_is_one(
        [(bls.g1_mul(bls.G1_GENERATOR, 6), bls.G2_GENERATOR), (bls.g1_neg(bls.G1_GENERATOR), bls.g2_mul(bls.G2_GENERATOR, 6))]
    )


def test_point_encodings_roundtrip_and_reject_non_canonical():
    for k in (1, 2, 7, 1 << 70):
        p1 = bls.g1_mul(bls.G1_GENERATOR, k)
        p2 = bls.g2_mul(bls.G2_GENERATOR, k)
        assert bls.g1_decompress(bls.g1_compress(p1)) == p1
        assert bls.g2_decompress(bls.g2_compress(p2)) == p2
        assert bls.g2_from_evm192(bls.g2_to_evm192(p2)) == p2
        assert len(bls.g1_to_eip2537(p1)) == 128 and len(bls.g2_to_eip2537(p2)) == 256
    assert bls.g1_decompress(bytes([0xC0]) + bytes(47)) is None
    for bad in (bytes([0xE0]) + bytes(47), bytes([0xC0]) + bytes(46) + b"\x01", bytes(48), bytes([0x9F]) + b"\xff" * 47):
        with pytest.raises(bls.PointEncodingError):
            bls.g1_decompress(bad)
    with pytest.raises(bls.PointEncodingError):
        bls.g2_from_evm192(bls.g2_to_evm192(bls.G2_GENERATOR)[:191] + b"\x00")


# -- hash to curve -----------------------------------------------------------------


def test_expand_message_xmd_rfc9380():
    for msg, length, uniform in EXPAND_MESSAGE_XMD_SHA256:
        assert expand_message_xmd(msg.encode(), EXPAND_MESSAGE_XMD_DST, length).hex() == uniform


def test_hash_to_g2_rfc9380():
    for vector in HASH_TO_G2_RO:
        trace = hash_to_g2_trace(vector["msg"].encode(), HASH_TO_G2_RO_DST)
        fp2 = lambda pair: (int(pair[0], 16), int(pair[1], 16))  # noqa: E731
        assert trace["u0"] == fp2(vector["u0"]) and trace["u1"] == fp2(vector["u1"])
        assert trace["q0"] == (fp2(vector["q0"][0]), fp2(vector["q0"][1]))
        assert trace["point"] == (fp2(vector["p"][0]), fp2(vector["p"][1]))
        assert bls.g2_add(trace["map_u0"], trace["map_u1"]) == trace["point"]


def test_sgn0():
    assert bls.f2_sgn0((0, 0)) == 0 and bls.f2_sgn0((1, 0)) == 1
    assert bls.f2_sgn0((0, 1)) == 1 and bls.f2_sgn0((2, 1)) == 0


# -- BLS signatures ----------------------------------------------------------------

# Values produced by `blst` 0.3.16 (`SecretKey::key_gen`, `sign` with DST_SIG) for the §11.3 test keys.
BLST_SK0 = "165e9e0625d23c83f6a34e786966cd88ae130231cf749c809c92d9437b6ee5d7"
BLST_PK0 = "98deef4a5d6468f0725ea03a677415de205c1c90ad01044187029cb9ad00cc7285e9b1d7ebd7ff6b2e3bddc7fe110d90"
BLST_PK2 = "b2d3a715d3fa2670e31baeecd4210431a8f09f5210e4a2e68c67083e38f7ac6f61134a4a0d9eaecf3bdd465d4533e470"
BLST_AGG21 = (
    "b0aca45f918a9ce7a8f15bc105b2adefd78b1b026dc92de440c2c90efbdce56fe9b5f748bf4b4e666234f6b54b179e9b"
    "17dea1cd9e2627b99e8bd8e9b544698e636f87b430754ff23a373e46b98a99f9e6f92553419c86b8796314502fef6181"
)


def _test_sk(i: int) -> int:
    return keygen(hashlib.sha256(b"sccp-finality-test-ikm" + bytes([i])).digest())


def test_keygen_and_aggregate_match_blst():
    assert _test_sk(0).to_bytes(32, "big").hex() == BLST_SK0
    assert sk_to_pk(_test_sk(0)).hex() == BLST_PK0 and sk_to_pk(_test_sk(2)).hex() == BLST_PK2
    msg = hashlib.sha256(b"sccp-reference-crosscheck").digest()
    sigs = [sign(_test_sk(i), msg) for i in range(21)]
    assert aggregate(sigs).hex() == BLST_AGG21
    assert fast_aggregate_verify([sk_to_pk(_test_sk(i)) for i in range(21)], msg, aggregate(sigs))


def test_sign_verify_and_validation():
    sk = _test_sk(1)
    msg = bytes(32)
    sig = sign(sk, msg)
    assert verify(sk_to_pk(sk), msg, sig)
    assert not verify(sk_to_pk(sk), b"\x01" * 32, sig)
    assert not verify(sk_to_pk(sk), msg, sign(sk, msg, b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_"))
    assert key_validate(bytes([0xC0]) + bytes(47)) is None
    assert signature_validate(bytes([0xC0]) + bytes(95)) is None
    assert signature_validate(sig) is not None


def test_aggregates():
    msgs = [bytes([i]) * 32 for i in range(3)]
    pks = [sk_to_pk(_test_sk(i)) for i in range(3)]
    agg = aggregate([sign(_test_sk(i), msgs[0]) for i in range(3)])
    assert fast_aggregate_verify(pks, msgs[0], agg)
    assert not fast_aggregate_verify(pks[:2], msgs[0], agg)
    assert not fast_aggregate_verify([], msgs[0], agg)
    distinct = aggregate([sign(_test_sk(i), msgs[i]) for i in range(3)])
    assert aggregate_verify(pks, msgs, distinct)
    assert not aggregate_verify(pks, [msgs[0], msgs[0], msgs[2]], distinct)


def test_consensus_digest_allowlist():
    commit = b"sumeragi/sig" + b"\x03" + bytes(153)
    assert consensus_digest(commit) == hashlib.sha256(commit).digest()
    assert consensus_digest(commit[:-1]) is None
    assert consensus_digest(b"sumeragi/sig" + b"\x06" + bytes(153)) is None
    assert consensus_digest(b"sumeragi/sig" + b"\x04" + bytes(88) + b"\x00") is not None
    assert consensus_digest(b"sumeragi/sig" + b"\x04" + bytes(88) + b"\x01") is None
    assert consensus_digest(b"sumeragi/availability/sign" + b"\x01" + bytes(192)) is not None


def test_ethereum_mainnet_sync_committee_aggregate():
    from sccp_reference.vectors_bls import ethereum_mainnet_vector

    vector = ethereum_mainnet_vector()
    assert vector["fast_aggregate_verify"] is True and vector["participants"] == 502


def test_rust_bls_consensus_fixture_agrees():
    """Rows of the Rust-generated `bls_consensus_rust_v1.json` (iroha_crypto) verify identically in Python."""
    path = FIXTURES / "bls_consensus_rust_v1.json"
    if not path.exists():
        pytest.skip("bls_consensus_rust_v1.json not generated yet")
    d = json.loads(path.read_text())
    sks = [int(k["secret_key_be"], 16) for k in d["keys"]]
    pks = [unhex(k["public_key"]) for k in d["keys"]]
    assert [sk_to_pk(sk) for sk in sks] == pks
    sv = d["standard_vectors"]
    for row in sv["sign"]:
        assert sign(sks[row["key"]], unhex(row["message"])) == unhex(row["signature"])
    for row in sv["verify"]:
        assert verify(pks[row["key"]], unhex(row["message"]), unhex(row["signature"])) == row["valid"]
    for row in sv["fast_aggregate_verify"]:
        keys = [pks[k] if isinstance(k, int) else unhex(k) for k in row["keys"]]
        assert fast_aggregate_verify(keys, unhex(row["message"]), unhex(row["signature"])) == row["valid"]
    for row in sv["aggregate_verify"]:
        keys = [pks[k] for k in row["keys"]]
        assert aggregate_verify(keys, [unhex(m) for m in row["messages"]], unhex(row["signature"])) == row["valid"]
    for row in d["allowlist"]:
        digest = consensus_digest(unhex(row["preimage"]))
        assert digest == unhex(row["digest"])
        assert [sign(sk, digest) for sk in sks] == [unhex(s) for s in row["signatures"]]
    for row in d["allowlist_rejections"]:
        assert consensus_digest(unhex(row["preimage"])) is None, row["case"]
    for row in d["negatives"]:
        if "key_validate" in row:
            assert key_validate(unhex(row["public_key"])) is None, row["case"]
        elif "consensus_valid" in row:
            digest = consensus_digest(unhex(row["preimage"])) if "preimage" in row else unhex(row["message"])
            keys = [unhex(k) for k in row.get("public_keys", [])] or [pks[row["key"]]]
            assert digest is None or not fast_aggregate_verify(keys, digest, unhex(row["signature"])), row["case"]


# -- payloads, leaves, trees, history -------------------------------------------------


def test_identity_words():
    assert identity_word(ETHEREUM, rep(0x11)) == bytes(31) + b"\x01"
    assert identity_word(TON, rep(0x11)).hex() == "ff" * 31 + "11"
    assert identity_word(TAIRA, rep(0x11)) == rep(0x11)


def test_payloads_and_message_ids_match_rust_payload_fixture():
    data = json.loads((FIXTURES / "payload_v1.json").read_text())
    network_id = unhex(data["taira_network_id"])
    checked = 0
    for direction in data["directions"]:
        if direction["source_domain"] not in BY_DOMAIN or direction["dest_domain"] not in BY_DOMAIN:
            continue  # the retired TRON domain
        payload = unhex(direction["payload"])
        decoded = decode_payload(payload)
        assert payload_hash(payload) == unhex(direction["payload_hash"])
        source, dest = BY_DOMAIN[decoded.source_domain], BY_DOMAIN[decoded.dest_domain]
        assert message_id(source, dest, network_id, payload) == unhex(direction["message_id"])
        checked += 1
    assert checked >= 6


def test_payload_rejections():
    sim = TairaSim(rep(0x11), rep(0x77), ETHEREUM, bytes(12) + rep(0x22, 20))
    payload = sim.transfer(0, 10**9, 1, rep(0x44, 20)).payload
    decode_payload(payload)
    for bad in (payload + b"\x00", payload[:-1], payload.replace(b"taira_eth_xor", b"taira_bsc_xor")):
        with pytest.raises(SccpError):
            decode_payload(bad)
    tron = bytearray(payload)
    tron[6:10] = (5).to_bytes(4, "big")
    with pytest.raises(SccpError):
        decode_payload(bytes(tron))


def test_commitment_tree_matches_rust_fixture():
    data = json.loads((FIXTURES / "commitment_tree_v1.json").read_text())
    leaves = [unhex(x) for x in data["leaves"]]
    for tree in data["trees"]:
        count = tree["count"]
        subset = leaves[:count]
        levels = tree_levels(subset)
        assert levels[-1][0] == unhex(tree["root"])
        for index, path in enumerate(tree["paths"]):
            assert [x.hex() for x in tree_path(subset, index, levels)] == [p[2:] for p in path]
            if index % 16 == 0 or index == count - 1:
                assert merkle_root(subset[index], index, count, [unhex(p) for p in path]) == unhex(tree["root"])
    example = data["examples"]["transfer"]
    assert transfer_leaf(unhex(example["message_id"]), unhex(example["destination_word"])) == unhex(example["leaf"])


def test_history_matches_rust_fixture_and_bagging():
    data = json.loads((FIXTURES / "history_v1.json").read_text())
    leaves = []
    for entry in data["leaves"]:
        leaf = history_leaf(entry["height"], unhex(entry["sccp_root"]), entry["message_count"])
        assert leaf == unhex(entry["leaf"])
        leaves.append(leaf)
    assert history_root([]) == bytes(32)
    for size in data["sizes"]:
        subset = leaves[: size["size"]]
        assert history_root(subset) == unhex(size["root"])
        assert [p.hex() for p in history_peaks(subset)] == [p[2:] for p in size["peaks"]]
        assert bag_peaks(history_peaks(subset)) == history_root(subset)


def test_merkle_root_rejects_malformed_paths():
    leaves = [rep(i) for i in range(1, 6)]
    path = tree_path(leaves, 1)
    assert merkle_root(leaves[1], 1, 5, path) == tree_root(leaves)
    for args in ((leaves[1], 1, 5, path[:-1]), (leaves[1], 1, 5, path + [rep(9)]), (leaves[1], 5, 5, path), (leaves[1], 0, 0, [])):
        with pytest.raises(SccpError):
            merkle_root(*args)
    assert tree_path(leaves, 4) == [tree_root(leaves[:4])]


def test_control_leaf_constraints():
    dw = bytes(12) + rep(0x22, 20)
    leaf = control_leaf(rep(0x11), ETHEREUM, dw, 1, 1, 1, 1, 0, rep(0x33), rep(0x44))
    assert leaf.hex() == "d9be284f6412d8e310b35afa921f8244f8d56cc64fbfc795d58701c11dce4d77"
    for args, error in (
        ((1, 1, 0, 1, 0, rep(0x33), rep(0x44)), "BadControl"),
        ((1, 1, 2, 0, 0, rep(0x33), rep(0x44)), "BadControl"),
        ((1, 1, 1, 2, 0, rep(0x33), rep(0x44)), "BadControl"),
        ((1, 1, 1, 1, 0, bytes(32), rep(0x44)), "BadControl"),
        ((0, 1, 1, 1, 0, rep(0x33), rep(0x44)), "ZeroRevision"),
        ((1, 0, 1, 1, 0, rep(0x33), rep(0x44)), "ZeroControlNonce"),
    ):
        with pytest.raises(SccpError) as err:
            control_leaf(rep(0x11), ETHEREUM, dw, *args)
        assert err.value.name == error


# -- finality header, R, P, anchors ---------------------------------------------------


def test_spec_worked_examples():
    from sccp_reference.vectors_finality import worked_examples

    examples = worked_examples()
    assert examples["commit"]["m"] == "0x2eca89b5065dd7b34b35d7273345c03257c2668c221499bac4afbdf0f2413e30"


def test_header_parse_invariants():
    from sccp_reference.vectors_finality import header_vectors

    vectors = header_vectors()
    assert {v["error"] for v in vectors["invalid"]} == {"X1", "X2", "X3", "X4", "X5", "X6", "X7", "Length"}
    for v in vectors["valid"]:
        assert parse_header(unhex(v["x"])).encode() == unhex(v["x"])


def test_inactive_header_and_result_of_preimage():
    x = inactive_header(rep(0x11), 9, 5).encode()
    assert x[65:] == bytes(156) and x[64] == 0
    assert result_of_preimage(x) is None
    assert result_of_preimage(x + b"\x00") is not None
    assert result_of_preimage(bytes(221) + b"\x00") is None


def test_qc_fixed_signers_and_quorum():
    assert signers_from_indices([0, 9, 30]) == 0x40000201
    assert bitmap_from_indices([0, 9, 30], 31).hex() == "01020040"
    assert [quorum(n) for n in (4, 7, 31)] == [3, 5, 21]
    assert not committee_n_valid(5) and committee_n_valid(31) and not committee_n_valid(34)
    p = commit_preimage(rep(0x77), 1, rep(0x88), 2, 3, rep(0x99), rep(0x66), 1)
    assert len(p) == 166 and p[12] == 3
    qc = parse_qc_fixed(bytes(113) + (0x40000201).to_bytes(4, "big"))
    assert qc.signers == 0x40000201


def test_anchor_chunk_paths_verify():
    leaves = [anchor_leaf(h, rep(0x99), rep(0x66)) for h in range(1, 4097)]
    levels = anchor_chunk_levels(leaves)
    path = anchor_chunk_path(levels, 3102)
    assert len(path) == 12
    h, pos = leaves[3102], 3102
    for sibling in path:
        h = anchor_node(sibling, h) if pos & 1 else anchor_node(h, sibling)
        pos >>= 1
    assert h == levels[-1][0]
    assert h_iroha(b"x")[-1] & 1 == 1


def test_certificate_aggregate_equals_sum_of_signatures():
    committee = Committee("t", (0, 1, 2, 3))
    header = FinalityHeader(network_id=rep(0x11), height=5, timestamp_ms=1, generation=1, committee_root=committee.root)
    cert = build_certificate(
        keys=committee.keys,
        secret_of=committee.secret_of,
        signer_indices=[0, 2, 3],
        header=header.encode(),
        instance=rep(0x77),
        epoch=0,
        epoch_context=rep(0x88),
        view=0,
        block_hash=rep(0x99),
        attest=1,
        result_body=rep(0x66),
    )
    m = cert.meta["m"]
    individual = aggregate([sign(committee.secret_of[committee.keys[i]], m) for i in (0, 2, 3)])
    assert individual == cert.signature_ton
    assert fast_aggregate_verify([committee.keys[i] for i in (0, 2, 3)], m, cert.signature_ton)
    assert hash_to_g2(m, DST_SIG) == cert.meta["hash_point"]


# -- destination model ------------------------------------------------------------------

T0 = 1_800_000_000_000


def _destination(flavor: str = "evm"):
    k0 = Committee("K0", (40, 41, 42, 43))
    sim = TairaSim(rep(0x11), rep(0x77), ETHEREUM, bytes(12) + rep(0x22, 20))
    sim.start(5, k0, 1000, T0, 1_209_600_000)
    deployment = Deployment(
        flavor, rep(0x11), rep(0x77), ETHEREUM, bytes(12) + rep(0x22, 20), 1, 10**18, 5, k0.root, 1000, T0 + 1_209_600_000, k0.keys
    )
    return sim, k0, Destination(deployment)


def _x(k0, height, flags=FLAG_ACTIVE, **fields):
    return FinalityHeader(network_id=rep(0x11), height=height, timestamp_ms=T0, flags=flags, generation=5, committee_root=k0.root, **fields)


@pytest.mark.parametrize(
    "first,second,reason",
    [
        (1010, ("conflict", 1010), 1),
        (None, ("plain", 999), 2),
        (1060, ("rotation", 1050), 3),
    ],
)
def test_violation_predicates_latch(first, second, reason):
    sim, k0, dest = _destination()
    if first is not None:
        assert dest.submit_checkpoints([sim.certify(_x(k0, first), k0)], T0)["outcome"] == "ok"
    kind, height = second
    if kind == "conflict":
        x = _x(k0, height, message_count=1, sccp_root=rep(0xEE))
    elif kind == "rotation":
        x = _x(k0, height, flags=FLAG_ACTIVE | FLAG_ROTATION, validity_ms=1, next_committee_root=k0.root)
    else:
        x = _x(k0, height)
    out = dest.submit_checkpoints([sim.certify(x, k0)], T0)
    assert out["outcome"] == "ok" and out["stopped"] == "latch"
    assert [e["reason"] for e in out["events"] if e["event"] == "SccpLatched"] == [reason]
    assert dest.state.equivocated and dest.state.until_ms == 0
    assert dest.void_frozen(0, 1, T0)["outcome"] == "ok"


def test_batch_is_atomic_and_nothing_to_do():
    sim, k0, dest = _destination()
    good = sim.certify(_x(k0, 1010), k0)
    bad = sim.certify(_x(k0, 1011), k0, signed_instance=rep(0x78))
    assert dest.submit_checkpoints([good, bad], T0)["outcome"] == "BadSignature"
    assert dest.state.checkpoints == {}
    assert dest.submit_checkpoints([good], T0)["outcome"] == "ok"
    assert dest.submit_checkpoints([good], T0)["outcome"] == "NothingToDo"
    assert dest.submit_checkpoints([good] * 17, T0)["outcome"] == "TooManyCertificates"


def test_rotation_deadline_is_anchored_at_t_eff():
    sim, k0, dest = _destination()
    k1 = Committee("K1", (44, 45, 46, 47))
    x = _x(k0, 1100, flags=FLAG_ACTIVE | FLAG_ROTATION, validity_ms=1000, next_committee_root=k1.root).with_(timestamp_ms=T0 + 50)
    assert dest.submit_checkpoints([sim.certify(x, k0)], T0 + 10)["outcome"] == "ok"
    assert (dest.state.generation, dest.state.start, dest.state.until_ms) == (6, 1101, T0 + 10 + 1000)


def test_ton_rotation_needs_next_keys():
    sim, k0, dest = _destination("ton")
    k1 = Committee("K1", (44, 45, 46, 47))
    x = _x(k0, 1100, flags=FLAG_ACTIVE | FLAG_ROTATION, validity_ms=1000, next_committee_root=k1.root)
    cert = sim.certify(x, k0)
    assert dest.submit_checkpoints([cert], T0, [None])["outcome"] == "BadCommittee"
    assert dest.submit_checkpoints([cert], T0, [k1.keys])["outcome"] == "ok"
    assert dest.state.keys == k1.keys


def test_check_block_ref_and_leaf():
    x = _x(Committee("K0", (40, 41, 42, 43)), 50, message_count=1, sccp_root=rep(0x21), history_size=3, history_root=rep(0x31))
    check_block_ref(50, rep(0x21), 1, 0, [], x)
    for args in ((50, rep(0x22), 1, 0, []), (51, rep(0x21), 1, 0, []), (40, rep(0x21), 0, 0, [])):
        with pytest.raises(SccpError):
            check_block_ref(*args, x)
    leaves = [rep(1), rep(2), rep(3)]
    check_leaf(leaves[2], 2, 3, tree_path(leaves, 2), tree_root(leaves))
    with pytest.raises(SccpError):
        check_leaf(leaves[2], 1, 3, tree_path(leaves, 2), tree_root(leaves))


def test_helper_functions():
    from sccp_reference.finality import (
        anchor_chunk_position,
        body_digest,
        canonical_order,
        checkpoint_id,
        forgery_evidence_id,
        indices_from_signers,
        y_sign_flag_matches,
    )
    from sccp_reference.merkle import control_leaf_preimage, control_leaf_ton_builders
    from sccp_reference.payload import lane_bytes, network_bytes

    f = bls.miller_loop(bls.G1_GENERATOR, bls.G2_GENERATOR)
    assert bls.f12_frobenius_p2(f) == bls.f12_pow(f, bls.P * bls.P)
    assert bls.fp_to_bytes64(5) == bytes(63) + b"\x05"
    rfc = HASH_TO_G2_RO[0]["p"]
    point = ((int(rfc[0][0], 16), int(rfc[0][1], 16)), (int(rfc[1][0], 16), int(rfc[1][1], 16)))
    assert bls.g2_uncompressed_zcash(point).hex().startswith(rfc[0][1][2:])
    assert len(network_bytes(TON, rep(0x11))) == 33 and len(lane_bytes(TAIRA, TON, rep(0x11))) == 66
    with pytest.raises(ValueError):
        lane_bytes(ETHEREUM, TON, rep(0x11))
    preimage = control_leaf_preimage(rep(0x11), ETHEREUM, bytes(32), 1, 1, 1, 0, 0, rep(1), rep(2))
    b1, b2 = control_leaf_ton_builders(preimage)
    assert (len(b1), len(b2)) == (125, 74) and keccak256(b1, b2) == keccak256(preimage)
    assert indices_from_signers(0x40000201) == [0, 9, 30]
    with pytest.raises(ValueError):
        canonical_order([rep(1, 48), rep(1, 48)])
    assert anchor_chunk_position(4096) == (0, 4095) and anchor_chunk_position(4097) == (1, 0)
    assert len(body_digest(b"")) == 32 and body_digest(b"")[-1] & 1
    assert checkpoint_id(bytes(221)) == keccak256(bytes(221))
    assert len(forgery_evidence_id(1, bytes(32), 7)) == 32
    pk = bls.g1_compress(bls.G1_GENERATOR)
    assert y_sign_flag_matches(pk, bls.G1_GENERATOR[1]) and not y_sign_flag_matches(pk, bls.P - bls.G1_GENERATOR[1])
    with pytest.raises(ValueError):
        keygen(bytes(31))


def test_destination_finalize_control_and_views():
    sim, k0, dest = _destination()
    recipient = rep(0x44, 20)
    block = sim.block(1010, T0, [sim.control(1, 2, 0, T0 + 500, rep(1), rep(2)), sim.transfer(0, 10**9, T0 + 1000, recipient)])
    cert = sim.certify(block.header, k0)
    assert dest.apply_control(("inline", cert), sim.control_proof(1010, 0, block.header), T0)["outcome"] == "ok"
    assert dest.pause_state(T0)["effective"] and not dest.pause_state(T0 + 500)["effective"]
    proof = sim.message_proof(1010, 1, block.header)
    assert dest.finalize(("checkpoint", block.x), proof, T0 + 100)["outcome"] == "MintingIsPaused"
    assert dest.finalize(("checkpoint", block.x), proof, T0 + 500)["outcome"] == "ok"
    assert dest.void_expired(0, ("checkpoint", block.x), proof, T0 + 1001)["outcome"] == "AlreadyConsumed"
    assert dest.verify_certificate(cert, T0)["outcome"] == "ok"
    assert dest.state.op_count == 2 and dest.state.total_supply == 10**9


def _fixture_certificate(entry: dict):
    from sccp_reference.finality import Certificate

    evm, ton = entry["evm"], entry["ton"]
    return Certificate(
        qc=unhex(evm["qc"]),
        header=unhex(evm["header"]),
        signature_evm=unhex(evm["signature"]),
        signature_ton=unhex(ton["signature"]),
        committee=unhex(evm["committee"]),
        signer_ys=unhex(evm["signerYs"]),
        meta={},
    )


def _fixture_destination(doc: dict, entry: dict, flavor: str) -> Destination:
    context, pin = doc["verification_context"], entry["pin"]
    keys = [unhex(k) for k in pin["keys"]] if "keys" in pin else [unhex(k) for k in doc["committees"][entry["committee"]]["keys"]]
    return Destination(
        Deployment(
            flavor,
            unhex(context["taira_network_id"]),
            unhex(context["consensus_instance"]),
            ETHEREUM,
            unhex(context["destination_word"]),
            1,
            10**18,
            pin["generation"],
            unhex(pin["committee_root"]),
            pin["start_height"],
            pin["until_ms"],
            keys,
        )
    )


def test_signer_set_negatives_reach_the_bitmap_check():
    """The popcount and spare-bit rows fail at §5.1.3 step 5 on EVM, not at the step 1 signerYs length."""
    doc = json.loads((FIXTURES / "finality_v1.json").read_text())
    rows = {entry["label"]: entry for entry in doc["negative_certificates"]}
    expected = {
        "popcount_q_minus_1": "popcount",
        "popcount_q_plus_1_extra_signer": "popcount",
        "spare_bits": "spare signer bits",
        "msb_first_bitmap": "spare signer bits",
    }
    for label, detail in expected.items():
        entry = rows[label]
        n = doc["committees"][entry["committee"]]["n"]
        assert len(unhex(entry["evm"]["signerYs"])) == 48 * quorum(n), label
        for flavor in ("evm", "ton"):
            out = _fixture_destination(doc, entry, flavor).verify_certificate(_fixture_certificate(entry), entry["now_ms"])
            assert (out["outcome"], out.get("detail")) == ("BadCertificate", detail), (label, flavor, out)


def test_ton_length_negatives_are_malformed_cells():
    doc = json.loads((FIXTURES / "finality_v1.json").read_text())
    rows = {entry["label"]: entry for entry in doc["negative_certificates"]}
    assert len(unhex(rows["qc_length_116"]["ton"]["qc_fixed_cell"])) == 112
    assert len(unhex(rows["header_length_220"]["ton"]["x_tail"])) == 111
    assert len(unhex(rows["ton_signature_length_95"]["ton"]["signature"])) == 95
    for entry in doc["certificates"]:
        ton = entry["ton"]
        assert len(unhex(ton["qc_fixed_cell"])) == 113 and len(unhex(ton["x_head"])) + len(unhex(ton["x_tail"])) == 221
        assert unhex(ton["qc_fixed_cell"]) + ton["signers"].to_bytes(4, "big") == unhex(entry["evm"]["qc"])


def test_every_fixture_outcome_is_a_named_error():
    doc = json.loads((FIXTURES / "committee_transitions_v1.json").read_text())
    known = {"ok", *doc["error_codes"]["shared"], *doc["error_codes"]["ton_only"]}
    control = json.loads((FIXTURES / "control_v1.json").read_text())
    for scenario in doc["scenarios"] + control["destination_effects"]:
        for step in scenario["steps"]:
            assert step["expected"].get("outcome", "ok") in known, (scenario["name"], step)
    assert doc["selectors"]["submitCheckpoints"]["selector"] == "0x07a5e706"
    assert doc["selectors"]["applyControl"]["selector"] == "0x32118e92"


def test_allowlist_rows_have_both_off_by_one_neighbours():
    doc = json.loads((FIXTURES / "bls_consensus_v1.json").read_text())
    rows = {row["label"]: row for row in doc["consensus_allowlist"]}
    positives = [label for label, row in rows.items() if row["allowed"]]
    assert len(positives) == 8
    for label in positives:
        for suffix, delta in (("_minus_1", -1), ("_plus_1", 1)):
            neighbour = rows[label + suffix]
            assert not neighbour["allowed"] and neighbour["len"] == rows[label]["len"] + delta
            assert consensus_digest(unhex(neighbour["preimage"])) is None


def test_taira_sim_orders_controls_first():
    sim, _, _ = _destination()
    with pytest.raises(ValueError):
        sim.block(1010, T0, [sim.transfer(0, 10**9, T0 + 1, rep(0x44, 20)), sim.control(1, 1, 1, 0, rep(1), rep(2))])


# -- fixtures ---------------------------------------------------------------------------


def test_render_is_canonical():
    assert generate.render({"b": 1, "a": [2]}) == '{\n  "a": [\n    2\n  ],\n  "b": 1\n}\n'


@pytest.mark.parametrize("name", sorted(generate.BUILDERS))
def test_fixture_has_no_drift(name):
    """Regenerate each fixture the reference owns; any difference is drift (rerun generate.py --write after review)."""
    diff = generate.check(name)
    assert not diff, "\n".join(diff)
