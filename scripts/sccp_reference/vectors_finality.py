"""Builder of `fixtures/sccp/finality_v1.json` (`specs/sccp.md` §3.6–§3.8, §11.2, §11.3)."""

from __future__ import annotations

import dataclasses

from .bls12_381 import (
    G1_GENERATOR,
    G2_GENERATOR,
    HALF_P,
    P,
    R,
    fp_to_bytes48,
    g1_compress,
    g1_decompress,
    g1_is_on_curve,
    g1_neg,
    g1_to_eip2537,
    g2_compress,
    g2_to_eip2537,
    g2_to_evm192,
)
from .common import MAX_HISTORY_SIZE, MAX_VALIDITY_MS, SccpError, hx, rep, sha256
from .destination import Deployment, Destination
from .finality import (
    COMMIT_PREIMAGE_LEN,
    FLAG_ACTIVE,
    FLAG_ROTATION,
    KIND_PREPARE,
    QC_FIXED_LEN,
    RESULT_BODY_TAG,
    RESULT_TAG,
    TAG_SIG,
    X_HEAD_LEN,
    X_LEN,
    X_MAGIC,
    FinalityHeader,
    QcFixed,
    anchor_chunk_levels,
    anchor_chunk_path,
    anchor_chunk_position,
    anchor_leaf,
    anchor_node,
    bitmap_from_indices,
    body_digest,
    checkpoint_id,
    commit_message,
    commit_preimage,
    committee_root,
    forgery_evidence_id,
    inactive_header,
    pairing_input,
    parse_header,
    quorum,
    result_of_preimage,
    result_r,
    signers_from_indices,
)
from .hash_to_curve import DST_SIG, iso_map_g2, map_to_curve_sswu
from .payload import ETHEREUM
from .taira import Committee, TairaSim, test_public, test_secret

NETWORK_ID = rep(0x11)
INSTANCE = rep(0x77)
T0 = 1_790_000_000_000
HOUR = 3_600_000
DAY = 86_400_000
VALIDITY = 1_209_600_000
GENERATION = 7
START_HEIGHT = 7000
DST_NUL = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_"

# §11.2 pinned values (asserted on every build).
SPEC_R = {
    "rotation": "3fcdcdfce761808f1a1f633fd209c197738d93032a0c5496ff20fb9bbcd783d7",
    "heartbeat": "ec15d2c034f21de8f6a6192bd246d5ee7acf99ae452c7a70f7c6003b567b1ed4",
    "non_rotation": "93d642e5cd5e489a395a937a88c63834fceaa8db9ef267e600f70ba58027c9ac",
    "inactive": "541b29c5e6058adb34edef96855cb321fdcdc635fc92e4ac25777d1b3ea07b1a",
}
SPEC_CHECKPOINT_ID = {
    "rotation": "b0a93737e03cec3ff52c920efa7db18859883ecb598ff7bdd2bf19f67c1086f6",
    "heartbeat": "0ae4bff13d50fc7129b3fcd5926df55d5da6afc8a33445abed1a1a7e650537a4",
    "non_rotation": "2d519a8ba8b000583ee3d0e6ec06843d95c3f7de32bf5ff9a76e6fad0fbbda6c",
}
SPEC_X_HEX = {
    "rotation": "534343502f46494e414c4954592f5631" + "11" * 32 + "0000000000001c20000001a0c4506c0003"
    "00000002" + "22" * 32 + "0000000000000005" + "33" * 32 + "0000000000000003" + "44" * 32
    + "0000000048190800" + "55" * 32,
    "non_rotation": "534343502f46494e414c4954592f5631" + "11" * 32 + "0000000000001c1f000001a0c450681801"
    "00000002" + "22" * 32 + "0000000000000005" + "33" * 32 + "0000000000000003" + "44" * 32
    + "0000000000000000" + "00" * 32,
}
SPEC_QC_FIXED_HEX = "0000000000000002" + "88" * 32 + "0000000000000001" + "99" * 32 + "01" + "66" * 32 + "40000201"
SPEC_M = "2eca89b5065dd7b34b35d7273345c03257c2668c221499bac4afbdf0f2413e30"
SPEC_FORGERY_ID = "c7970cdfc9d3bdb5098d0a7e8e06763b7a2f451b09c9ef9fa5168d20db3c0ca9"
SPEC_ANCHOR_7199 = "a22346dda5d3ec12cd1308908cbbcb509f68f3b860377153ca4bae4a5f13df65"
SPEC_ANCHOR_7200 = "0422761979d4e89d78edd5dcfbd719b1f6e10f339603a53da14e4b1587202ed7"
SPEC_ANCHOR_NODE = "1d1e5d3ee82efa64b0c2703e18202b34c29bed398a688bb1626f873c2d36d1a3"
SPEC_CHUNK0_ROOT = "2367ae00d979ce576c55f1be87c6971c22921aae16c939eb9ff85b820d2e235f"
SPEC_SYNTHETIC_COMMITTEE_ROOT = "fe8902a93a6592a748b8528a9f49312e9cbc5d1abcccb1ff7431b0acba92b2b9"
NEG_G1_Y = 0x114D1D6855D545A8AA7D76C8CF2E21F267816AEF1DB507C96655B9D5CAAC42364E6F38BA0ECB751BAD54DCD6B939C2CA
HALF_P_SPEC = 0x0D0088F51CBFF34D258DD3DB21A5D66BB23BA5C279C2895FB39869507B587B120F55FFFF58A9FFFFDCFF7FFFFFFFD555


def _check(actual: bytes, expected_hex: str, what: str) -> None:
    if actual.hex() != expected_hex:
        raise AssertionError(f"{what}: computed {actual.hex()} differs from the specification value {expected_hex}")


def g1_json(pt) -> dict:
    return {"compressed": hx(g1_compress(pt)), "eip2537": hx(g1_to_eip2537(pt))}


def g2_json(pt) -> dict:
    return {"compressed": hx(g2_compress(pt)), "evm192": hx(g2_to_evm192(pt)), "eip2537": hx(g2_to_eip2537(pt))}


def constants_json() -> dict:
    neg_g1 = g1_neg(G1_GENERATOR)
    assert neg_g1[1] == NEG_G1_Y and HALF_P == HALF_P_SPEC
    return {
        "x_magic": {"ascii": X_MAGIC.decode(), "hex": hx(X_MAGIC)},
        "x_len": X_LEN,
        "x_head_len": X_HEAD_LEN,
        "x_tail_len": X_LEN - X_HEAD_LEN,
        "result_tag": {"ascii": RESULT_TAG.decode(), "hex": hx(RESULT_TAG)},
        "result_body_tag": {"ascii": RESULT_BODY_TAG.decode(), "hex": hx(RESULT_BODY_TAG)},
        "tag_sig": {"ascii": TAG_SIG.decode(), "hex": hx(TAG_SIG)},
        "dst_sig": {"ascii": DST_SIG.decode(), "hex": hx(DST_SIG), "len": len(DST_SIG)},
        "qc_fixed_len": QC_FIXED_LEN,
        "commit_preimage_len": COMMIT_PREIMAGE_LEN,
        "committee_tag": "SCCP/COMMITTEE/V1",
        "max_validity_ms": MAX_VALIDITY_MS,
        "max_history_size": str(MAX_HISTORY_SIZE),
        "bls12_381": {
            "p": hex(P),
            "half_p": hex(HALF_P),
            "r": hex(R),
            "g1": g1_json(G1_GENERATOR),
            "neg_g1": g1_json(neg_g1),
            "neg_g1_y": hex(NEG_G1_Y),
            "g2": g2_json(G2_GENERATOR),
        },
    }


def worked_examples() -> dict:
    """§11.2, every value asserted against the specification."""
    common = dict(
        network_id=rep(0x11),
        message_count=2,
        sccp_root=rep(0x22),
        history_size=5,
        history_root=rep(0x33),
        generation=3,
        committee_root=rep(0x44),
    )
    d_body = rep(0x66)
    rotation = FinalityHeader(
        height=7200, timestamp_ms=1_790_000_000_000, flags=3, validity_ms=1_209_600_000, next_committee_root=rep(0x55), **common
    )
    heartbeat = rotation.with_(next_committee_root=rep(0x44))
    non_rotation = FinalityHeader(height=7199, timestamp_ms=1_789_999_999_000, flags=1, **common)
    inactive = inactive_header(rep(0x11), 9, 1_790_000_000_000)
    cases = {}
    for name, header in (
        ("rotation", rotation),
        ("heartbeat", heartbeat),
        ("non_rotation", non_rotation),
        ("inactive", inactive),
    ):
        x = header.encode()
        parse_header(x)
        r = result_r(x, d_body)
        _check(r, SPEC_R[name], f"R({name})")
        entry = {"fields": header.as_json(), "x": hx(x), "d_body": hx(d_body), "r": hx(r)}
        if name in SPEC_CHECKPOINT_ID:
            _check(checkpoint_id(x), SPEC_CHECKPOINT_ID[name], f"checkpoint_id({name})")
            entry["checkpoint_id"] = hx(checkpoint_id(x))
        if name in SPEC_X_HEX:
            _check(x, SPEC_X_HEX[name], f"X({name})")
        cases[name] = entry
    qc = QcFixed(2, rep(0x88), 1, rep(0x99), 1, rep(0x66), signers_from_indices([0, 9, 30]))
    _check(qc.encode(), SPEC_QC_FIXED_HEX, "QC_FIXED")
    r_rot = result_r(rotation.encode(), d_body)
    preimage = commit_preimage(INSTANCE, 2, rep(0x88), 7200, 1, rep(0x99), r_rot, 1)
    m = commit_message(preimage)
    _check(m, SPEC_M, "m")
    fid = forgery_evidence_id(3, m, 0x40000201)
    _check(fid, SPEC_FORGERY_ID, "forgery evidence id")
    r_non = result_r(non_rotation.encode(), d_body)
    a7199 = anchor_leaf(7199, rep(0x99), r_non)
    a7200 = anchor_leaf(7200, rep(0x99), r_rot)
    _check(a7199, SPEC_ANCHOR_7199, "anchor_leaf(7199)")
    _check(a7200, SPEC_ANCHOR_7200, "anchor_leaf(7200)")
    pair = anchor_node(a7199, a7200)
    _check(pair, SPEC_ANCHOR_NODE, "anchor node")
    assert anchor_chunk_position(7199) == (1, 3102) and anchor_chunk_position(7200) == (1, 3103)
    chunk0 = anchor_chunk_levels([anchor_leaf(h, rep(0x99), rep(0x66)) for h in range(1, 4097)])
    _check(chunk0[-1][0], SPEC_CHUNK0_ROOT, "chunk 0 root")
    synthetic = committee_root([rep(1, 48), rep(2, 48), rep(3, 48), rep(4, 48)])
    _check(synthetic, SPEC_SYNTHETIC_COMMITTEE_ROOT, "synthetic committee root")
    return {
        "cases": cases,
        "qc_fixed": {
            "fields": qc.as_json(),
            "signer_indices": [0, 9, 30],
            "bitmap": hx(bitmap_from_indices([0, 9, 30], 31)),
            "encoded": hx(qc.encode()),
            "raw_bitmap_signers_negative": hex(int.from_bytes(bitmap_from_indices([0, 9, 30], 31), "big")),
        },
        "commit": {
            "instance": hx(INSTANCE),
            "epoch": 2,
            "epoch_context": hx(rep(0x88)),
            "height": 7200,
            "view": 1,
            "block_hash": hx(rep(0x99)),
            "r": hx(r_rot),
            "attest": 1,
            "preimage": hx(preimage),
            "m": hx(m),
        },
        "forgery_evidence_id": {"generation": 3, "m": hx(m), "signers": 0x40000201, "id": hx(fid)},
        "anchors": {
            "core_hash": hx(rep(0x99)),
            "leaf_7199": {"chunk": 1, "position": 3102, "r": hx(r_non), "leaf": hx(a7199)},
            "leaf_7200": {"chunk": 1, "position": 3103, "r": hx(r_rot), "leaf": hx(a7200)},
            "node_7199_7200": hx(pair),
            "chunk0_all_99_66": {
                "root": hx(chunk0[-1][0]),
                "path_height_1": [hx(h) for h in anchor_chunk_path(chunk0, 0)],
                "path_height_4096": [hx(h) for h in anchor_chunk_path(chunk0, 4095)],
                "leaf_height_1": hx(chunk0[0][0]),
                "leaf_height_4096": hx(chunk0[0][4095]),
            },
        },
        "synthetic_committee_root": {
            "keys": ["0x" + "01" * 48, "0x" + "02" * 48, "0x" + "03" * 48, "0x" + "04" * 48],
            "root": hx(synthetic),
        },
    }


def header_vectors() -> dict:
    base = FinalityHeader(
        network_id=NETWORK_ID,
        height=7100,
        timestamp_ms=T0,
        flags=FLAG_ACTIVE,
        message_count=3,
        sccp_root=rep(0x21),
        history_size=12,
        history_root=rep(0x31),
        generation=GENERATION,
        committee_root=rep(0x41),
    )
    rotation = base.with_(flags=3, validity_ms=VALIDITY, next_committee_root=rep(0x51))
    valid = {
        "non_rotation": base,
        "rotation": rotation,
        "heartbeat": rotation.with_(next_committee_root=base.committee_root),
        "empty_block": base.with_(message_count=0, sccp_root=bytes(32)),
        "empty_history": base.with_(message_count=0, sccp_root=bytes(32), history_size=0, history_root=bytes(32)),
        "maximal": rotation.with_(
            message_count=512,
            history_size=MAX_HISTORY_SIZE,
            validity_ms=MAX_VALIDITY_MS,
            height=(1 << 53) - 1,
            timestamp_ms=(1 << 53) - 1,
        ),
        "inactive": inactive_header(NETWORK_ID, 7100, T0),
        "inactive_height_1": inactive_header(NETWORK_ID, 1, 0),
    }
    out_valid = []
    for label, header in valid.items():
        x = header.encode()
        parsed = parse_header(x)
        out_valid.append(
            {
                "label": label,
                "fields": header.as_json(),
                "x": hx(x),
                "x_head": hx(x[:X_HEAD_LEN]),
                "x_tail": hx(x[X_HEAD_LEN:]),
                "checkpoint_id": hx(checkpoint_id(x)),
                "active": parsed.active,
                "rotation": parsed.rotation,
                "destination": "accept" if parsed.active else "BadHeader",
            }
        )
    bad_magic = bytearray(base.encode())
    bad_magic[0] ^= 0x01
    invalid = {
        "magic": ("X1", bytes(bad_magic)),
        "flag_bit_2": ("X1", base.with_(flags=0x05).encode()),
        "flag_bit_7": ("X1", base.with_(flags=0x81).encode()),
        "flags_0xc0": ("X1", base.with_(flags=0xC1).encode()),
        "inactive_with_message_count": ("X2", inactive_header(NETWORK_ID, 7100, T0).with_(message_count=1, sccp_root=rep(0x21)).encode()),
        "inactive_with_generation": ("X2", inactive_header(NETWORK_ID, 7100, T0).with_(generation=1).encode()),
        "inactive_with_rotation": ("X2", inactive_header(NETWORK_ID, 7100, T0).with_(flags=FLAG_ROTATION).encode()),
        "generation_zero": ("X3", base.with_(generation=0).encode()),
        "committee_root_zero": ("X3", base.with_(committee_root=bytes(32)).encode()),
        "count_without_root": ("X4", base.with_(sccp_root=bytes(32)).encode()),
        "root_without_count": ("X4", base.with_(message_count=0).encode()),
        "count_513": ("X4", base.with_(message_count=513).encode()),
        "history_size_without_root": ("X5", base.with_(history_root=bytes(32)).encode()),
        "history_root_without_size": ("X5", base.with_(history_size=0).encode()),
        "history_size_2_32_plus_1": ("X5", base.with_(history_size=MAX_HISTORY_SIZE + 1).encode()),
        "rotation_validity_zero": ("X6", rotation.with_(validity_ms=0).encode()),
        "rotation_validity_above_max": ("X6", rotation.with_(validity_ms=MAX_VALIDITY_MS + 1).encode()),
        "rotation_next_root_zero": ("X6", rotation.with_(next_committee_root=bytes(32)).encode()),
        "non_rotation_with_validity": ("X6", base.with_(validity_ms=1).encode()),
        "non_rotation_with_next_root": ("X6", base.with_(next_committee_root=rep(0x51)).encode()),
        "height_zero": ("X7", base.with_(height=0).encode()),
        "inactive_height_zero": ("X7", inactive_header(NETWORK_ID, 0, T0).encode()),
        "length_220": ("Length", base.encode()[:-1]),
        "length_222": ("Length", base.encode() + b"\x00"),
    }
    out_invalid = []
    for label, (error, data) in invalid.items():
        try:
            parse_header(data)
        except SccpError as err:
            assert err.name == error, (label, err.name, error)
        else:
            raise AssertionError(f"{label} parsed")
        out_invalid.append({"label": label, "x": hx(data), "error": error, "destination": "BadHeader"})
    return {"valid": out_valid, "invalid": out_invalid}


def result_of_preimage_vectors() -> list[dict]:
    x = FinalityHeader(
        network_id=NETWORK_ID, height=7100, timestamp_ms=T0, generation=GENERATION, committee_root=rep(0x41)
    ).encode()
    inactive = inactive_header(NETWORK_ID, 7100, T0).encode()
    rows = []
    for label, body in (("one_byte_body", b"\x00"), ("short_body", b"sccp-reference body"), ("inactive_x", None)):
        preimage = (inactive if body is None else x) + (b"\x01\x02\x03" if body is None else body)
        result = result_of_preimage(preimage)
        rows.append(
            {
                "label": label,
                "preimage": hx(preimage),
                "d_body": hx(body_digest(preimage[X_LEN:])),
                "r": hx(result),
            }
        )
    max_body = bytes(65_315)
    max_r = result_of_preimage(x + max_body)
    assert max_r is not None and result_of_preimage(x + max_body + b"\x00") is None
    rows.append(
        {
            "label": "maximal_body",
            "construction": "the X of row one_byte_body followed by 65315 zero bytes (65536 bytes in total)",
            "preimage_len": X_LEN + 65_315,
            "d_body": hx(body_digest(max_body)),
            "r": hx(max_r),
        }
    )
    rows.append(
        {
            "label": "body_too_long",
            "construction": "the X of row one_byte_body followed by 65316 zero bytes (65537 bytes)",
            "preimage_len": X_LEN + 65_316,
            "r": None,
        }
    )
    for label, preimage in (("x_only", x), ("truncated_x", x[:200]), ("unparseable_x", bytes(X_LEN) + b"\x00")):
        assert result_of_preimage(preimage) is None
        rows.append({"label": label, "preimage": hx(preimage), "r": None})
    return rows


def signer_encoding_vectors() -> list[dict]:
    rows = []
    for label, n, indices in (
        ("n4_lowest", 4, [0, 1, 2]),
        ("n4_highest", 4, [1, 2, 3]),
        ("n7_lowest", 7, [0, 1, 2, 3, 4]),
        ("n7_highest", 7, [2, 3, 4, 5, 6]),
        ("n31_example", 31, [0, 9, 30]),
        ("n31_lowest", 31, list(range(21))),
        ("n31_highest", 31, list(range(10, 31))),
    ):
        bitmap = bitmap_from_indices(indices, n)
        signers = signers_from_indices(indices)
        rows.append(
            {
                "label": label,
                "n": n,
                "indices": indices,
                "bitmap": hx(bitmap),
                "signers": signers,
                "signers_be32": hx(signers.to_bytes(4, "big")),
                "raw_bitmap_misreading": hex(int.from_bytes(bitmap.ljust(4, b"\x00")[:4], "big")) if n > 8 else None,
            }
        )
    return rows


def signer_ys_exactly_q(cert, keys: list[bytes]):
    """`cert` with `signerYs` of exactly `48q` bytes, so that EVM passes the step 1 length check.

    The `y` values of the encoded in-range signers come first (ascending index); a short list
    is padded with the `y` of the lowest keys outside the encoded set, a long one truncated.
    """
    q = quorum(len(keys))
    signers = int.from_bytes(cert.qc[113:117], "big")
    ys = cert.signer_ys
    for i in range(len(keys)):
        if len(ys) >= 48 * q:
            break
        if not signers >> i & 1:
            ys += fp_to_bytes48(g1_decompress(keys[i])[1])
    return dataclasses.replace(cert, signer_ys=ys[: 48 * q])


def _pin(committee: Committee, generation: int = GENERATION) -> dict:
    return {
        "generation": generation,
        "committee_root": committee.root,
        "start_height": START_HEIGHT,
        "until_ms": T0 + VALIDITY,
        "keys": committee.keys,
    }


def _deployment(flavor: str, pin: dict) -> Deployment:
    return Deployment(
        flavor=flavor,
        taira_network_id=NETWORK_ID,
        consensus_instance=INSTANCE,
        network=ETHEREUM,
        destination_word=bytes(12) + rep(0x22, 20),
        route_revision=1,
        max_wrapped_supply=10**18,
        pin_generation=pin["generation"],
        pin_root=pin["committee_root"],
        pin_start_height=pin["start_height"],
        pin_until_ms=pin["until_ms"],
        pin_keys=pin["keys"],
    )


def _outcome(flavor: str, pin: dict, cert, now_ms: int) -> str:
    return Destination(_deployment(flavor, pin)).verify_certificate(cert, now_ms)["outcome"]


def _certificate_json(label, description, committee_name, cert, pin, now_ms, *, evm=True, ton=True, detail=True) -> dict:
    meta = cert.meta
    expected = {
        "evm": _outcome("evm", pin, cert, now_ms) if evm else None,
        "ton": _outcome("ton", pin, cert, now_ms) if ton else None,
    }
    try:
        header_fields = parse_header(cert.header).as_json()
    except SccpError:
        header_fields = None
    entry = {
        "label": label,
        "description": description,
        "committee": committee_name,
        "pin": {
            "generation": pin["generation"],
            "committee_root": hx(pin["committee_root"]),
            "start_height": pin["start_height"],
            "until_ms": pin["until_ms"],
        },
        "now_ms": now_ms,
        "header_fields": header_fields,
        "signer_indices": meta["signer_indices"],
        "bitmap": hx(meta["bitmap"]),
        "qc_fixed_fields": meta["qc"].as_json(),
        "commit_kind": meta["kind"],
        "dst": meta["dst"].decode(),
        "r": hx(meta["r"]),
        "commit_preimage": hx(meta["p"]),
        "m": hx(meta["m"]),
        "checkpoint_id": hx(checkpoint_id(cert.header)) if len(cert.header) == X_LEN else None,
        "evm": cert.evm_json(),
        "ton": cert.ton_json(),
        "expected": expected,
    }
    if detail:
        entry["hash_to_g2_m"] = g2_json(meta["hash_point"])
        entry["aggregate_public_key"] = g1_json(meta["apk"])
        entry["aggregate_signature"] = g2_json(meta["sigma"])
        entry["pairing_input"] = hx(pairing_input(meta["apk"], meta["hash_point"], meta["sigma"]))
    if len(cert.header) == X_LEN and len(cert.qc) == QC_FIXED_LEN:
        x_gen = int.from_bytes(cert.header[141:149], "big")
        entry["forgery_evidence_id"] = hx(forgery_evidence_id(x_gen, meta["m"], int.from_bytes(cert.qc[113:117], "big")))
    if pin.get("custom_keys"):
        entry["pin"]["keys"] = [hx(k) for k in pin["keys"]]
    return entry


def build() -> dict:
    """The complete `finality_v1.json` document."""
    committees = {
        "n4": Committee("n4", tuple(range(4))),
        "n7": Committee("n7", tuple(range(7))),
        "n31": Committee("n31", tuple(range(31))),
    }
    other4 = Committee("n4_other", tuple(range(31, 35)))
    sim = TairaSim(NETWORK_ID, INSTANCE, ETHEREUM, bytes(12) + rep(0x22, 20))
    now = T0 + HOUR

    def header(committee: Committee, height: int, **changes) -> FinalityHeader:
        return FinalityHeader(
            network_id=NETWORK_ID,
            height=height,
            timestamp_ms=T0 + (height - START_HEIGHT) * 1000,
            flags=FLAG_ACTIVE,
            message_count=2,
            sccp_root=sha256(b"sccp-reference/block-root", height.to_bytes(8, "big")),
            history_size=40,
            history_root=sha256(b"sccp-reference/history-root", height.to_bytes(8, "big")),
            generation=GENERATION,
            committee_root=committee.root,
        ).with_(**changes)

    positives = []
    scattered = {
        "n4": [0, 1, 3],
        "n7": [0, 2, 3, 5, 6],
        "n31": [i for i in range(31) if i % 3 != 1],
    }
    for name, committee in committees.items():
        q = quorum(committee.n)
        pin = _pin(committee)
        for set_label, indices in (
            ("lowest", list(range(q))),
            ("highest", list(range(committee.n - q, committee.n))),
            ("scattered", scattered[name]),
        ):
            x = header(committee, 7100 + len(positives))
            cert = sim.certify(x, committee, signer_indices=indices)
            positives.append(
                _certificate_json(
                    f"{name}_{set_label}_q", f"non-rotation certificate, {set_label} {q} signers of n = {committee.n}", name, cert, pin, now
                )
            )
    assert 0 in scattered["n31"] and 9 in scattered["n31"] and 30 in scattered["n31"]
    n4 = committees["n4"]
    pin4 = _pin(n4)
    kinds = (
        ("key_change_rotation", header(n4, 7200, flags=3, validity_ms=VALIDITY, next_committee_root=other4.root), 1),
        ("heartbeat_rotation", header(n4, 7201, flags=3, validity_ms=VALIDITY, next_committee_root=n4.root), 1),
        ("rotation_attest_0", header(n4, 7202, flags=3, validity_ms=VALIDITY, next_committee_root=other4.root), 0),
        ("non_rotation_attest_0", header(n4, 7203), 0),
        ("non_rotation_attest_1", header(n4, 7204), 1),
        ("empty_block_non_rotation", header(n4, 7205, message_count=0, sccp_root=bytes(32)), 1),
    )
    for label, x, attest in kinds:
        cert = sim.certify(x, n4, attest=attest)
        positives.append(_certificate_json(label, f"{label.replace('_', ' ')} (n = 4)", "n4", cert, pin4, now))
    for entry in positives:
        assert entry["expected"] == {"evm": "ok", "ton": "ok"}, entry["label"]

    negatives = _negatives(sim, committees, other4, header, now)
    return {
        "schema": "iroha.sccp.finality.v1",
        "spec": "specs/sccp.md §3.6–§3.8, §5.1.3, §11.2, §11.3 (revision 5)",
        "generator": "scripts/sccp_reference/generate.py (independent Python reference)",
        "conventions": "byte strings are 0x-prefixed lowercase hex; field elements and scalars that exceed 2^53 are hex strings; "
        "`expected.evm`/`expected.ton` name the §5.2.2 outcome of VerifyCertificate steps 1–12 on a fresh destination "
        "deployed with `pin` at `now_ms` (null where that flavor cannot express the vector)",
        "constants": constants_json(),
        "worked_examples": worked_examples(),
        "headers": header_vectors(),
        "result_of_preimage": result_of_preimage_vectors(),
        "signer_encodings": signer_encoding_vectors(),
        "test_keys": [
            {
                "index": i,
                "ikm": hx(sha256(b"sccp-finality-test-ikm", bytes([i]))),
                "sk": hx(test_secret(i).to_bytes(32, "big")),
                "pk": hx(test_public(i)),
                "pk_eip2537": hx(g1_to_eip2537(g1_decompress(test_public(i)))),
                "pk_y": hx(fp_to_bytes48(g1_decompress(test_public(i))[1])),
            }
            for i in range(35)
        ],
        "committees": {name: c.as_json() for name, c in {**committees, "n4_other": other4}.items()},
        "verification_context": {
            "taira_network_id": hx(NETWORK_ID),
            "consensus_instance": hx(INSTANCE),
            "network": ETHEREUM.name,
            "destination_word": hx(bytes(12) + rep(0x22, 20)),
        },
        "certificates": positives,
        "negative_certificates": negatives,
        "todo": [
            "TODO: a w3f-transcript signature over a consensus digest (§11.3) needs the iroha_crypto w3f "
            "transcript; the Rust reference adds it when it regenerates this file",
            "TODO: crates/iroha_sccp (Rust reference) regenerates this file and must match it byte for byte",
        ],
    }


def _negatives(sim: TairaSim, committees: dict, other4: Committee, header, now: int) -> list[dict]:
    n4, n7, n31 = committees["n4"], committees["n7"], committees["n31"]
    pin4 = _pin(n4)
    out = []

    def add(label, description, cert, pin=pin4, committee="n4", *, evm=True, ton=True, expect=None):
        entry = _certificate_json(label, description, committee, cert, pin, now, evm=evm, ton=ton, detail=False)
        if expect is not None:
            for flavor, outcome in expect.items():
                if entry["expected"][flavor] != outcome:
                    raise AssertionError(f"{label}/{flavor}: model gave {entry['expected'][flavor]}, expected {outcome}")
        out.append(entry)

    base_x = header(n4, 7300)
    good = sim.certify(base_x, n4)

    add("prepare_kind", "PrepareQC (kind 0x02) instead of CommitQC", sim.certify(header(n4, 7301), n4, kind=KIND_PREPARE),
        expect={"evm": "BadSignature", "ton": "BadSignature"})
    # The signer-set rows carry exactly 48q bytes of signerYs, so EVM passes the step 1 length
    # check and reaches the step 5 bitmap check they exist for (`signer_ys_short` pins step 1).
    add("popcount_q_minus_1", "q − 1 signers (signerYs padded to 48q with a non-signer's y)",
        signer_ys_exactly_q(sim.certify(header(n4, 7302), n4, signer_indices=[0, 1]), n4.keys),
        expect={"evm": "BadCertificate", "ton": "BadCertificate"})
    add("popcount_q_plus_1_extra_signer", "q + 1 signers, an extra signer (signerYs truncated to 48q)",
        signer_ys_exactly_q(sim.certify(header(n4, 7303), n4, signer_indices=[0, 1, 2, 3]), n4.keys),
        expect={"evm": "BadCertificate", "ton": "BadCertificate"})
    spare = sim.certify(header(n4, 7304), n4, signer_indices=[0, 1, 2], signers_field=0b10011)
    add("spare_bits", "a signer bit at index n (spare bit) with popcount q (signerYs of 48q bytes)",
        signer_ys_exactly_q(spare, n4.keys), expect={"evm": "BadCertificate", "ton": "BadCertificate"})
    add("attest_2", "attest = 2", sim.certify(header(n4, 7305), n4, attest=2),
        expect={"evm": "BadCertificate", "ton": "BadCertificate"})
    add("wrong_instance", "signed over a Commit preimage with another instance I",
        sim.certify(header(n4, 7306), n4, signed_instance=rep(0x78)), expect={"evm": "BadSignature", "ton": "BadSignature"})
    add("wrong_network", "X.network_id differs from the destination's Taira NetworkId",
        sim.certify(header(n4, 7307, network_id=rep(0x12)), n4), expect={"evm": "BadHeader", "ton": "BadHeader"})
    add("wrong_dst", "signed with the NUL ciphersuite DST instead of DST_SIG",
        sim.certify(header(n4, 7308), n4, dst=DST_NUL), expect={"evm": "BadSignature", "ton": "BadSignature"})
    add("signed_height_mismatch", "the Commit preimage names X.height + 1",
        sim.certify(header(n4, 7309), n4, signed_height=7310), expect={"evm": "BadSignature", "ton": "BadSignature"})
    x_signed = header(n4, 7310)
    tampered = sim.certify(x_signed, n4)
    tampered = dataclasses.replace(tampered, header=x_signed.with_(sccp_root=rep(0xEE)).encode())
    add("tampered_x", "a valid-layout X changed after signing (different sccp_root)", tampered,
        expect={"evm": "BadSignature", "ton": "BadSignature"})
    wrong_subset = sim.certify(header(n4, 7311), n4, signer_indices=[0, 1, 2], signing_keys=[n4.keys[1], n4.keys[2], n4.keys[3]])
    add("signature_of_other_subset", "the bitmap names {0,1,2} but {1,2,3} signed", wrong_subset,
        expect={"evm": "BadSignature", "ton": "BadSignature"})

    # Dirty X (each signed by the committee, so only the header check rejects it).
    dirty = {
        "x1_magic": None,
        "x1_flags_0x04": header(n4, 7320, flags=0x05),
        "x2_inactive_with_fields": inactive_header(NETWORK_ID, 7321, T0).with_(generation=GENERATION),
        "inactive_x": inactive_header(NETWORK_ID, 7322, T0),
        "x3_generation_zero": header(n4, 7323, generation=0),
        "x4_count_without_root": header(n4, 7324, sccp_root=bytes(32)),
        "x4_count_513": header(n4, 7325, message_count=513),
        "x5_history_size_without_root": header(n4, 7326, history_root=bytes(32)),
        "x6_rotation_validity_zero": header(n4, 7327, flags=3, validity_ms=0, next_committee_root=other4.root),
        "x6_non_rotation_with_validity": header(n4, 7328, validity_ms=VALIDITY),
        "x6_non_rotation_with_next_root": header(n4, 7329, next_committee_root=other4.root),
        "x7_height_zero": header(n4, 0),
    }
    for label, x in dirty.items():
        if x is None:
            raw = bytearray(header(n4, 7330).encode())
            raw[3] ^= 0x20
            raw = bytes(raw)
        else:
            raw = x.encode()
        cert = sim.certify(raw, n4)
        add(f"dirty_{label}", f"dirty X ({label.replace('_', ' ')}), validly signed", cert,
            expect={"evm": "BadHeader", "ton": "BadHeader"})

    # Classification.
    add("past_generation", "a genuine-looking certificate of generation g − 1 (same keys)",
        sim.certify(header(n4, 7340, generation=GENERATION - 1), n4), expect={"evm": "CommitteeNotAccepted", "ton": "CommitteeNotAccepted"})
    add("foreign_committee_root", "X.committee_root names another committee",
        sim.certify(header(other4, 7341), other4), committee="n4_other",
        expect={"evm": "CommitteeNotAccepted", "ton": "CommitteeNotAccepted"})

    # EVM committee and signerYs inputs.
    root_mismatch = dataclasses.replace(good, committee=b"".join(other4.keys))
    add("committee_root_mismatch", "calldata committee whose keccak root differs from cur.root", root_mismatch, ton=False,
        expect={"evm": "BadCommittee"})
    keys = n4.keys
    ys = [g1_decompress(k)[1] for k in keys]

    def with_y(cert, index_in_signers, y_value):
        sy = bytearray(cert.signer_ys)
        sy[48 * index_in_signers : 48 * index_in_signers + 48] = y_value.to_bytes(48, "big")
        return dataclasses.replace(cert, signer_ys=bytes(sy))

    add("signer_y_wrong_sign", "signerYs[0] = p − y, inconsistent with the key's sign bit", with_y(good, 0, P - ys[0]), ton=False,
        expect={"evm": "BadCommittee"})
    add("signer_y_ge_p", "signerYs[1] = y + p", with_y(good, 1, ys[1] + P), ton=False, expect={"evm": "BadCommittee"})
    off = ys[2]
    for delta in range(1, 64):
        candidate = (off + delta) % P
        if (candidate > HALF_P) == (ys[2] > HALF_P) and not g1_is_on_curve((g1_decompress(keys[2])[0], candidate)):
            off = candidate
            break
    add("signer_y_off_curve", "signerYs[2] has the right sign class but is not on the curve", with_y(good, 2, off), ton=False,
        expect={"evm": "BadCommittee"})
    for label, flag_fn in (("key_flag_infinity_0xc0", lambda b: b | 0x40), ("key_flag_uncompressed", lambda b: b & 0x7F)):
        bad_keys = list(keys)
        bad_keys[0] = bytes([flag_fn(bad_keys[0][0])]) + bad_keys[0][1:]
        bad_root = committee_root(bad_keys)
        pin = {**pin4, "committee_root": bad_root, "keys": bad_keys, "custom_keys": True}
        x = header(n4, 7350 if "infinity" in label else 7351, committee_root=bad_root)
        cert = sim.certify(x, n4)
        cert = dataclasses.replace(cert, committee=b"".join(bad_keys))
        add(label, "committee key 0 with a bad flag byte, pinned under the root of those exact bytes", cert, pin=pin, ton=False,
            expect={"evm": "BadCommittee"})

    # Signature encodings.
    sigma = good.meta["sigma"]
    q0 = iso_map_g2(map_to_curve_sswu((5, 7)))
    from .bls12_381 import g2_in_subgroup  # local: only needed here

    assert q0 is not None and not g2_in_subgroup(q0)
    add("sigma_not_in_subgroup", "σ on the curve but outside G2",
        dataclasses.replace(good, signature_evm=g2_to_evm192(q0), signature_ton=g2_compress(q0)),
        expect={"evm": "BadSignature", "ton": "BadSignature"})
    add("sigma_zero", "σ = 0^192 (EVM) / the compressed identity (TON)",
        dataclasses.replace(good, signature_evm=bytes(192), signature_ton=bytes([0xC0]) + bytes(95)),
        expect={"evm": "BadSignature", "ton": "BadSignature"})
    evm = g2_to_evm192(sigma)
    for position, limb_name in enumerate(("x_c0", "x_c1", "y_c0", "y_c1")):
        limb = bytearray(evm)
        value = int.from_bytes(evm[48 * position : 48 * position + 48], "big") + P
        limb[48 * position : 48 * position + 48] = value.to_bytes(48, "big")
        add(f"sigma_limb_{limb_name}_ge_p", f"σ.{limb_name.replace('_', '.')} + p in the EVM form",
            dataclasses.replace(good, signature_evm=bytes(limb)), ton=False, expect={"evm": "BadSignature"})
    swapped = evm[48:96] + evm[0:48] + evm[144:192] + evm[96:144]
    add("sigma_c0_c1_swapped", "EVM σ with c1 ‖ c0 limb order (ZCash order mistaken for EIP-2537)",
        dataclasses.replace(good, signature_evm=swapped), ton=False, expect={"evm": "BadSignature"})
    off_curve = bytearray(evm)
    off_curve[191] ^= 0x01
    add("sigma_off_curve", "EVM σ with y.c1 changed by one bit", dataclasses.replace(good, signature_evm=bytes(off_curve)), ton=False,
        expect={"evm": "BadSignature"})
    ton_bad = bytearray(g2_compress(sigma))
    ton_bad[0] &= 0x7F
    add("sigma_ton_uncompressed_flag", "TON σ without the compression flag", dataclasses.replace(good, signature_ton=bytes(ton_bad)),
        evm=False, expect={"ton": "BadSignature"})

    # Bitmap order (n = 31 scattered set, and n = 7).
    idx31 = [i for i in range(31) if i % 3 != 1]
    raw_misread = int.from_bytes(bitmap_from_indices(idx31, 31), "big")
    pin31 = _pin(n31)
    add("raw_bitmap_signers", "signers = the Sumeragi bitmap bytes read big-endian (n = 31)",
        sim.certify(header(n31, 7360), n31, signer_indices=idx31, signers_field=raw_misread), pin=pin31, committee="n31",
        expect={"evm": "BadSignature", "ton": "BadSignature"})
    idx7 = [0, 1, 2, 3, 4]
    msb = 0
    for i in idx7:
        msb |= 1 << (8 * (i // 8) + 7 - i % 8)
    add("msb_first_bitmap", "signers with MSB-first bit order inside the byte (n = 7; signerYs of 48q bytes)",
        signer_ys_exactly_q(sim.certify(header(n7, 7361), n7, signer_indices=idx7, signers_field=msb), n7.keys),
        pin=_pin(n7), committee="n7", expect={"evm": "BadCertificate", "ton": "BadCertificate"})

    # Exact lengths.
    add("qc_length_116", "QC_FIXED of 116 bytes", dataclasses.replace(good, qc=good.qc[:-1]),
        expect={"evm": "BadCertificate", "ton": "BadCertificate"})
    add("header_length_220", "X of 220 bytes", dataclasses.replace(good, header=good.header[:-1]),
        expect={"evm": "BadHeader", "ton": "BadHeader"})
    add("evm_signature_length_191", "EVM signature of 191 bytes", dataclasses.replace(good, signature_evm=good.signature_evm[:-1]),
        ton=False, expect={"evm": "BadCertificate"})
    add("ton_signature_length_95", "TON signature of 95 bytes", dataclasses.replace(good, signature_ton=good.signature_ton[:-1]),
        evm=False, expect={"ton": "BadCertificate"})
    add("committee_n_5", "calldata committee of 5 keys", dataclasses.replace(good, committee=good.committee + keys[0]),
        ton=False, expect={"evm": "BadCommittee"})
    add("signer_ys_short", "signerYs of 48(q − 1) bytes", dataclasses.replace(good, signer_ys=good.signer_ys[:-48]),
        ton=False, expect={"evm": "BadCertificate"})
    return out
